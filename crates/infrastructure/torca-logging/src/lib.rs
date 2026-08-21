//! Structured, redaction-safe JSONL logging shared by native Torca runtimes.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_MESSAGE_LENGTH: usize = 512;
// Diagnostics must stay useful on a phone without turning into a storage or
// write-amplification workload.  Incident export is the escape hatch for a
// longer history; the on-device rolling window is intentionally bounded.
const MAX_LOCAL_RUNS: usize = 5;
const MAX_LOCAL_BYTES: u64 = 64 * 1024 * 1024;
const LOG_DOMAINS: [&str; 9] =
    ["runtime", "bootstrap", "tor", "relay", "storage", "profile", "messaging", "ffi", "ui"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// One process/run logger. Each domain is written to its own JSONL `.log` file.
pub struct Logger {
    root: PathBuf,
    date: String,
    run_id: String,
    device_id: String,
    build_id: String,
    started_ms: u128,
    finished: AtomicBool,
    writes_since_flush: AtomicUsize,
    files: Mutex<Vec<(String, File)>>,
}

/// Platform default readable by the diagnostic collector without exposing private data.
pub fn default_root() -> PathBuf {
    if let Some(root) = std::env::var_os("TORCA_LOG_ROOT") {
        return root.into();
    }
    if std::env::consts::OS == "windows" {
        if let Some(root) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(root).join("Torca").join("logs");
        }
    }
    if std::env::consts::OS == "android" {
        PathBuf::from("/sdcard/Android/data/com.torca.torca_app/files/torca/logs")
    } else {
        std::env::temp_dir().join("Torca").join("logs")
    }
}

impl Logger {
    /// Creates `root/devices/device/date/run-000001` and writes `run.start.json`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the run directory or its manifest cannot be written.
    pub fn new(
        root: impl AsRef<Path>,
        device_id: impl Into<String>,
        build_id: impl Into<String>,
    ) -> std::io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        let date = utc_date();
        let device_id = sanitize_component(&device_id.into());
        let day_root = root.join("devices").join(&device_id).join(&date);
        fs::create_dir_all(&day_root)?;
        let run_number = next_run_number(&day_root);
        let run_id = format!("run-{run_number:06}");
        let directory = day_root.join(&run_id);
        fs::create_dir_all(&directory)?;
        let build_id = redact(&build_id.into());
        let started_ms = now_ms();
        let logger = Self {
            root,
            date,
            run_id,
            device_id,
            build_id,
            started_ms,
            finished: AtomicBool::new(false),
            writes_since_flush: AtomicUsize::new(0),
            files: Mutex::new(Vec::new()),
        };
        {
            let mut files =
                logger.files.lock().map_err(|_| std::io::Error::other("logger mutex poisoned"))?;
            for domain in LOG_DOMAINS {
                let path = logger.directory().join(format!("{domain}.log"));
                let file = OpenOptions::new().create(true).append(true).open(path)?;
                files.push((domain.to_owned(), file));
            }
        }
        logger.write_json_file("run.start.json", &format!(
            "{{\"schema\":1,\"status\":\"running\",\"started_at_ms\":{},\"run_id\":\"{}\",\"incident_id\":\"{}\",\"device_id\":\"{}\",\"build_id\":\"{}\",\"platform\":\"{}\"}}\n",
            started_ms,
            escape(&logger.run_id),
            escape(&logger.run_id),
            escape(&logger.device_id),
            escape(&logger.build_id),
            platform_name()))?;
        logger.enforce_retention();
        Ok(logger)
    }

    /// Writes one redaction-safe structured event to `<domain>.log`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the domain log cannot be opened or flushed.
    pub fn event(
        &self,
        domain: &str,
        level: Level,
        component: &str,
        code: &str,
        message: &str,
    ) -> std::io::Result<()> {
        self.event_with_context(domain, level, component, code, message, None)
    }

    /// Writes an event with an already serialized, non-sensitive JSON context object.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the domain log cannot be opened or flushed.
    pub fn event_with_context(
        &self,
        domain: &str,
        level: Level,
        component: &str,
        code: &str,
        message: &str,
        context_json: Option<&str>,
    ) -> std::io::Result<()> {
        let domain = sanitize_component(domain);
        let context = context_json.map_or_else(|| "{}".into(), redact);
        let line = format!(
            "{{\"schema\":1,\"ts_ms\":{},\"level\":\"{}\",\"run_id\":\"{}\",\"incident_id\":\"{}\",\"device_id\":\"{}\",\"build_id\":\"{}\",\"domain\":\"{}\",\"component\":\"{}\",\"code\":\"{}\",\"message\":\"{}\",\"context\":{}}}\n",
            now_ms(),
            level.as_str(),
            escape(&self.run_id),
            escape(&self.run_id),
            escape(&self.device_id),
            escape(&self.build_id),
            domain,
            escape(component),
            escape(&code.to_ascii_uppercase()),
            escape(&redact(message)),
            context
        );
        let mut files =
            self.files.lock().map_err(|_| std::io::Error::other("logger mutex poisoned"))?;
        let index = files.iter().position(|(name, _)| name == &domain);
        let file = if let Some(index) = index {
            &mut files[index].1
        } else {
            let path = self.directory().join(format!("{domain}.log"));
            let file = OpenOptions::new().create(true).append(true).open(path)?;
            files.push((domain.clone(), file));
            let Some((_, file)) = files.last_mut() else {
                return Err(std::io::Error::other("logger file was not inserted"));
            };
            file
        };
        file.write_all(line.as_bytes())?;
        // Flushing every JSON event turns diagnostics into a synchronous
        // fsync-like workload on Android. Keep the stream visible in normal
        // operation, but batch flushes and always flush on `finish`.
        let writes = self.writes_since_flush.fetch_add(1, Ordering::Relaxed) + 1;
        if writes.is_multiple_of(32) {
            file.flush()?;
        }
        Ok(())
    }

    /// Writes a small redaction-safe JSON snapshot next to the domain logs.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the snapshot cannot be written.
    pub fn write_json_file(&self, name: &str, content: &str) -> std::io::Result<()> {
        let safe_name = sanitize_file_name(name);
        let path = self.directory().join(safe_name);
        let mut file = File::create(path)?;
        file.write_all(redact(content).as_bytes())
    }

    /// Marks the run as completed or interrupted and writes `run.end.json`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the final manifest cannot be written.
    pub fn finish(&self, status: &str, reason: &str) -> std::io::Result<()> {
        if self.finished.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let status = sanitize_component(status);
        let reason = redact(reason);
        let duration = now_ms().saturating_sub(self.started_ms);
        if let Ok(mut files) = self.files.lock() {
            for (_, file) in &mut *files {
                let _ = file.flush();
            }
        }
        self.write_json_file("run.end.json", &format!(
            "{{\"schema\":1,\"status\":\"{}\",\"ended_at_ms\":{},\"duration_ms\":{},\"run_id\":\"{}\",\"incident_id\":\"{}\",\"reason\":\"{}\"}}\n",
            status,
            now_ms(),
            duration,
            escape(&self.run_id),
            escape(&self.run_id),
            escape(&reason)))
    }

    pub fn directory(&self) -> PathBuf {
        self.root.join("devices").join(&self.device_id).join(&self.date).join(&self.run_id)
    }
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    fn enforce_retention(&self) {
        let device_root = self.root.join("devices").join(&self.device_id);
        let mut runs = Vec::new();
        if let Ok(dates) = fs::read_dir(&device_root) {
            for date in dates.flatten() {
                if let Ok(entries) = fs::read_dir(date.path()) {
                    for run in entries
                        .flatten()
                        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
                    {
                        if run.file_name().to_string_lossy().starts_with("run-") {
                            runs.push(run.path());
                        }
                    }
                }
            }
        }
        runs.sort();
        let mut bytes = runs.iter().map(|path| directory_size(path)).sum::<u64>();
        while runs.len() > MAX_LOCAL_RUNS || bytes > MAX_LOCAL_BYTES {
            if runs.len() <= 1 {
                break;
            }
            let oldest = runs.remove(0);
            if oldest == self.directory() {
                continue;
            }
            let size = directory_size(&oldest);
            if fs::remove_dir_all(&oldest).is_ok() {
                bytes = bytes.saturating_sub(size);
            }
        }
    }
}

impl Drop for Logger {
    fn drop(&mut self) {
        let _ = self.finish("interrupted", "logger dropped before explicit completion");
    }
}

fn next_run_number(day_root: &Path) -> u32 {
    fs::read_dir(day_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_prefix("run-")?.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}
fn directory_size(path: &Path) -> u64 {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| {
            let p = entry.path();
            if p.is_dir() { directory_size(&p) } else { entry.metadata().map_or(0, |m| m.len()) }
        })
        .sum()
}
fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |value| value.as_millis())
}
fn utc_date() -> String {
    civil_date(u64::try_from(now_ms() / 1000 / 86_400).unwrap_or(u64::MAX))
}
fn civil_date(days: u64) -> String {
    let mut year = 1970i64;
    let mut remaining = i64::try_from(days).unwrap_or(i64::MAX);
    loop {
        let length = if is_leap(year) { 366 } else { 365 };
        if remaining < length {
            break;
        }
        remaining -= length;
        year += 1;
    }
    let mut month = 1i64;
    while remaining >= month_days(year, month) {
        remaining -= month_days(year, month);
        month += 1;
    }
    format!("{year:04}-{month:02}-{day:02}", day = remaining + 1)
}
fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn month_days(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
fn platform_name() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(windows) {
        "windows"
    } else {
        "unknown"
    }
}
fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(
            |ch| if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') { ch } else { '_' },
        )
        .collect::<String>()
        .trim_matches('_')
        .chars()
        .take(80)
        .collect::<String>()
        .if_empty()
}
fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(
            |ch| if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') { ch } else { '_' },
        )
        .collect::<String>()
        .trim_matches('_')
        .chars()
        .take(120)
        .collect::<String>()
        .if_empty()
}
fn escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            ch if ch.is_control() => "?".chars().collect(),
            ch => vec![ch],
        })
        .collect()
}
fn redact(value: &str) -> String {
    let mut output = value.to_owned();
    for needle in [
        "private_key",
        "private-key",
        "secret=",
        "capability=",
        "token=",
        "password=",
        "plaintext=",
    ] {
        loop {
            let lower = output.to_ascii_lowercase();
            let Some(index) = lower.find(needle) else { break };
            output.replace_range(index.., "[REDACTED]");
        }
    }
    output.chars().take(MAX_MESSAGE_LENGTH).collect()
}

trait EmptyString {
    fn if_empty(self) -> String;
}
impl EmptyString for String {
    fn if_empty(self) -> String {
        if self.is_empty() { "unknown".into() } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::{Level, Logger};
    #[test]
    fn writes_target_layout_manifest_and_redacted_jsonl() {
        let root = std::env::temp_dir().join(format!("torca-logging-{}", std::process::id()));
        let logger = Logger::new(&root, "test device", "build").expect("logger");
        logger
            .event("runtime", Level::Error, "tor", "tor_failed", "secret=private-key")
            .expect("event");
        logger.finish("completed", "ok").expect("finish");
        let device =
            std::fs::read_dir(root.join("devices")).expect("devices").next().unwrap().unwrap();
        let date = std::fs::read_dir(device.path()).unwrap().next().unwrap().unwrap();
        let run = std::fs::read_dir(date.path()).unwrap().next().unwrap().unwrap();
        let content = std::fs::read_to_string(run.path().join("runtime.log")).expect("content");
        assert!(content.contains("REDACTED"));
        assert!(!content.contains("private-key"));
        assert!(run.path().join("run.start.json").exists());
        assert!(run.path().join("run.end.json").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
