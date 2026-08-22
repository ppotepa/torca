//! Ratatui launcher and live monitor for the physical Android battery soak.
//!
//! The measurement remains owned by `Run-TorcaBatterySoak.ps1`; this binary
//! only supervises it and samples independent ADB evidence while it runs.

use std::collections::VecDeque;
use std::io::{self, BufRead, IsTerminal, Stdout};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

#[derive(Clone, Debug, Parser)]
#[command(name = "torca-battery-soak-tui")]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    #[arg(long, default_value_t = 60)]
    duration_minutes: u64,
    #[arg(long)]
    device_id: String,
    #[arg(long, default_value = "com.torca.torca_app")]
    package: String,
    #[arg(long)]
    output_root: Option<PathBuf>,
    #[arg(long)]
    require_unplugged: bool,
    #[arg(long)]
    require_screen_off: bool,
    #[arg(long)]
    collect_native_diagnostics: bool,
    #[arg(long, default_value_t = 2)]
    auto_deploy_attempts: u8,
    #[arg(long, default_value = ".")]
    repo_root: PathBuf,
    /// Run the PowerShell harness without the terminal dashboard.
    #[arg(long)]
    plain: bool,
}

#[derive(Default)]
struct Telemetry {
    source: String,
    level: String,
    wakefulness: String,
    pid: String,
    installed: String,
    last_sample: String,
}

struct State {
    started: Instant,
    lines: VecDeque<String>,
    telemetry: Telemetry,
    artifact: String,
    status: String,
    cancelled: bool,
}

enum OutputLine {
    Stdout(String),
    Stderr(String),
}

fn main() {
    if let Err(error) = run() {
        eprintln!("torca-battery-soak-tui: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    if args.duration_minutes == 0 {
        return Err("duration-minutes must be positive".into());
    }
    if args.auto_deploy_attempts == 0 || args.auto_deploy_attempts > 3 {
        return Err("auto-deploy-attempts must be between 1 and 3".into());
    }
    if args.plain || !io::stdout().is_terminal() || !io::stderr().is_terminal() {
        if !args.plain {
            eprintln!("torca-battery-soak-tui: non-interactive output detected; using plain mode");
        }
        return run_plain(&args);
    }
    run_tui(args)
}

fn script_args(args: &Args) -> Vec<String> {
    let mut values = vec![
        "-NoLogo".into(),
        "-NoProfile".into(),
        "-ExecutionPolicy".into(),
        "Bypass".into(),
        "-File".into(),
        args.repo_root.join("scripts/Run-TorcaBatterySoak.ps1").to_string_lossy().into_owned(),
        "-DurationMinutes".into(),
        args.duration_minutes.to_string(),
        "-DeviceId".into(),
        args.device_id.clone(),
        "-Package".into(),
        args.package.clone(),
        "-AutoDeployAttempts".into(),
        args.auto_deploy_attempts.to_string(),
    ];
    if let Some(output) = &args.output_root {
        values.extend(["-OutputRoot".into(), output.to_string_lossy().into_owned()]);
    }
    if args.require_unplugged {
        values.push("-RequireUnplugged".into());
    }
    if args.require_screen_off {
        values.push("-RequireScreenOff".into());
    }
    if args.collect_native_diagnostics {
        values.push("-CollectNativeDiagnostics".into());
    }
    values.push("-ValidateAfter".into());
    values
}

fn powershell() -> &'static str {
    if cfg!(windows) { "powershell" } else { "pwsh" }
}

fn spawn_harness(
    args: &Args,
    piped: bool,
) -> Result<(Child, Option<Receiver<OutputLine>>), String> {
    let mut command = Command::new(powershell());
    command.current_dir(&args.repo_root).args(script_args(args));
    if piped {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }
    let mut child = command.spawn().map_err(|error| {
        format!(
            "start PowerShell harness with '{}': {error}",
            args.repo_root.join("scripts/Run-TorcaBatterySoak.ps1").display()
        )
    })?;
    if !piped {
        return Ok((child, None));
    }
    let stdout = child.stdout.take().ok_or("PowerShell stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("PowerShell stderr unavailable")?;
    let (tx, rx) = mpsc::channel();
    spawn_reader(stdout, tx.clone(), false);
    spawn_reader(stderr, tx, true);
    Ok((child, Some(rx)))
}

fn spawn_reader<R: io::Read + Send + 'static>(reader: R, tx: Sender<OutputLine>, stderr: bool) {
    thread::spawn(move || {
        let reader = io::BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            let _ =
                tx.send(if stderr { OutputLine::Stderr(line) } else { OutputLine::Stdout(line) });
        }
    });
}

fn run_plain(args: &Args) -> Result<(), String> {
    let (mut child, _) = spawn_harness(args, false)?;
    let status = child.wait().map_err(|error| format!("wait for battery soak: {error}"))?;
    if status.success() { Ok(()) } else { Err(format!("battery soak exited with {status}")) }
}

fn run_tui(args: Args) -> Result<(), String> {
    let (mut child, receiver) = spawn_harness(&args, true)?;
    let receiver = receiver.ok_or("battery soak output receiver unavailable")?;
    let mut terminal = match setup_terminal() {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            wake_device(&args.device_id);
            return Err(format!("setup terminal: {error}"));
        }
    };
    let mut state = State {
        started: Instant::now(),
        lines: VecDeque::new(),
        telemetry: Telemetry::default(),
        artifact: "pending".into(),
        status: "starting".into(),
        cancelled: false,
    };
    state.telemetry = sample_adb(&args.device_id, &args.package);
    let loop_result = (|| -> Result<(), String> {
        let mut last_sample = Instant::now();
        loop {
            while let Ok(line) = receiver.try_recv() {
                consume_line(&mut state, line);
            }
            if last_sample.elapsed() >= Duration::from_secs(2) {
                state.telemetry = sample_adb(&args.device_id, &args.package);
                last_sample = Instant::now();
            }
            if let Some(status) =
                child.try_wait().map_err(|error| format!("poll soak process: {error}"))?
            {
                state.status = if status.success() { "completed" } else { "failed" }.into();
                break;
            }
            if event::poll(Duration::from_millis(100))
                .map_err(|error| format!("poll key: {error}"))?
            {
                if let Event::Key(
                    KeyEvent { code: KeyCode::Char('q'), .. } | KeyEvent { code: KeyCode::Esc, .. },
                ) = event::read().map_err(|error| format!("read key: {error}"))?
                {
                    state.cancelled = true;
                    state.status = "cancelling".into();
                    let _ = child.kill();
                    wake_device(&args.device_id);
                }
            }
            terminal
                .draw(|frame| draw(frame, &state, &args))
                .map_err(|error| format!("draw dashboard: {error}"))?;
        }
        Ok(())
    })();
    let _ = child.wait();
    let teardown =
        teardown_terminal(&mut terminal).map_err(|error| format!("teardown terminal: {error}"));
    if let Err(error) = loop_result {
        let _ = teardown;
        return Err(error);
    }
    teardown?;
    if state.cancelled {
        return Ok(());
    }
    if state.status == "failed" {
        return Err("battery soak failed; inspect the live output and artifact log".into());
    }
    Ok(())
}

fn consume_line(state: &mut State, line: OutputLine) {
    let (prefix, line) = match line {
        OutputLine::Stdout(line) => ("", line),
        OutputLine::Stderr(line) => ("ERR ", line),
    };
    if let Some(path) = line.strip_prefix("Battery soak capture complete: ") {
        state.artifact = path.trim().into();
    }
    state.lines.push_back(format!("{prefix}{line}"));
    while state.lines.len() > 80 {
        state.lines.pop_front();
    }
    if line.contains("auto-deploy") {
        state.status = "deploying".into();
    }
    if line.contains("Battery preflight") {
        state.status = "preflight".into();
    }
}

fn sample_adb(device: &str, package: &str) -> Telemetry {
    let battery = adb(device, &["shell", "dumpsys", "battery"]);
    let power = adb(device, &["shell", "dumpsys", "power"]);
    let pid = adb(device, &["shell", "pidof", package]);
    let installed = adb(device, &["shell", "pm", "path", package]);
    let source = battery_source(&battery);
    let level = battery.lines().find_map(|line| line.trim().strip_prefix("level: ")).unwrap_or("?");
    let wakefulness = power
        .lines()
        .find(|line| line.contains("mWakefulness="))
        .unwrap_or("unknown")
        .trim()
        .to_owned();
    Telemetry {
        source: source.into(),
        level: level.into(),
        wakefulness,
        pid: if pid.trim().is_empty() { "not running".into() } else { pid.trim().into() },
        installed: if installed.contains("package:") {
            "installed".into()
        } else {
            "missing".into()
        },
        last_sample: chrono_like_now(),
    }
}

fn battery_source(text: &str) -> &'static str {
    if text.contains("AC powered: true") {
        "ac"
    } else if text.contains("USB powered: true") {
        "usb"
    } else if text.contains("Wireless powered: true") {
        "wireless"
    } else {
        "battery"
    }
}

fn chrono_like_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| format!("{}ms", value.as_millis()))
        .unwrap_or_else(|_| "unknown".into())
}

fn adb(device: &str, args: &[&str]) -> String {
    Command::new("adb")
        .args(["-s", device])
        .args(args)
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

fn wake_device(device: &str) {
    let _ = Command::new("adb")
        .args(["-s", device, "shell", "input", "keyevent", "KEYCODE_WAKEUP"])
        .status();
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

fn draw(frame: &mut ratatui::Frame, state: &State, args: &Args) {
    let area = frame.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(7),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " TORCA BATTERY SOAK ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " {}  elapsed={}s/{}m",
            state.status,
            state.started.elapsed().as_secs(),
            args.duration_minutes
        )),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Session"));
    frame.render_widget(header, rows[0]);
    let telemetry = vec![
        ListItem::new(format!("Device: {}", args.device_id)),
        ListItem::new(format!(
            "Power: {}  level={}  screen={}",
            state.telemetry.source, state.telemetry.level, state.telemetry.wakefulness
        )),
        ListItem::new(format!(
            "Package: {}  PID: {}",
            state.telemetry.installed, state.telemetry.pid
        )),
        ListItem::new(format!("Artifact: {}", state.artifact)),
        ListItem::new(format!("ADB sample: {}", state.telemetry.last_sample)),
    ];
    frame.render_widget(
        List::new(telemetry).block(Block::default().borders(Borders::ALL).title("ADB telemetry")),
        rows[1],
    );
    let logs = state
        .lines
        .iter()
        .rev()
        .take(rows[2].height.saturating_sub(2) as usize)
        .cloned()
        .map(ListItem::new)
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(logs).block(Block::default().borders(Borders::ALL).title("Harness output")),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new("q / Esc: stop harness and wake device")
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::DarkGray)),
        rows[3],
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn power_source_parser_distinguishes_charging_states() {
        assert_eq!(super::battery_source("AC powered: true"), "ac");
        assert_eq!(super::battery_source("USB powered: true"), "usb");
        assert_eq!(super::battery_source("Wireless powered: true"), "wireless");
        assert_eq!(super::battery_source("AC powered: false\nUSB powered: false"), "battery");
    }
}
