//! Interactive terminal dashboard for the soak runner.

use std::collections::VecDeque;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph};
use serde_json::Value;

use crate::Cli;

const MAX_EVENTS: usize = 200;

struct UiContext {
    cancel: AtomicBool,
    paused: AtomicBool,
    incident: AtomicBool,
    retry: AtomicBool,
    started: Instant,
    run_id: Mutex<Option<String>>,
    phase: Mutex<String>,
    last_event: Mutex<String>,
    android_status: Mutex<String>,
    relay_endpoint: Mutex<String>,
    relay_health: Mutex<String>,
    onion_state: Mutex<String>,
    events: Mutex<VecDeque<String>>,
    logs: Mutex<VecDeque<String>>,
    run_root: Mutex<Option<PathBuf>>,
    message_count: std::sync::atomic::AtomicUsize,
    attachment_count: std::sync::atomic::AtomicUsize,
    radio_count: std::sync::atomic::AtomicUsize,
    ready_peers: std::sync::atomic::AtomicUsize,
}

static ACTIVE: OnceLock<Mutex<Option<Arc<UiContext>>>> = OnceLock::new();

fn active() -> Option<Arc<UiContext>> {
    ACTIVE.get()?.lock().ok()?.clone()
}

pub(crate) fn cancel_requested() -> bool {
    active().is_some_and(|ctx| ctx.cancel.load(Ordering::Acquire))
}

pub(crate) fn take_retry_requested() -> bool {
    active().is_some_and(|ctx| ctx.retry.swap(false, Ordering::AcqRel))
}

pub(crate) fn set_run_root(root: &Path) {
    let Some(ctx) = active() else { return };
    if let Ok(mut run_root) = ctx.run_root.lock() {
        *run_root = Some(root.to_owned());
    }
    if let Ok(mut run_id) = ctx.run_id.lock() {
        *run_id = root.file_name().map(|value| value.to_string_lossy().into_owned());
    }
}

pub(crate) fn controlled_sleep(duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if cancel_requested() {
            return;
        }
        if let Some(ctx) = active() {
            while ctx.paused.load(Ordering::Acquire) && !ctx.cancel.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(100));
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}

pub(crate) fn publish_event(event: &str, line: &Value) {
    let Some(ctx) = active() else { return };
    let summary = if let Some(data) = line.get("data") {
        format!("{event} {}", compact(data))
    } else {
        event.to_owned()
    };
    if let Ok(mut last_event) = ctx.last_event.lock() {
        event.clone_into(&mut last_event);
    }
    if let Ok(mut phase) = ctx.phase.lock() {
        event_phase(event).clone_into(&mut phase);
    }
    if let Some(data) = line.get("data") {
        if let Some(serial) = data.get("serial").and_then(Value::as_str) {
            if let Ok(mut status) = ctx.android_status.lock() {
                if event.contains("ready") {
                    "ready".clone_into(&mut status);
                } else {
                    "working".clone_into(&mut status);
                }
                if event == "android_preflight_started" {
                    *status = format!("preflight ({serial})");
                }
            }
        }
        if event == "relay_ready" {
            if let Some(endpoint) = data.get("endpoint").and_then(Value::as_str) {
                if let Ok(mut value) = ctx.relay_endpoint.lock() {
                    endpoint.clone_into(&mut value);
                }
            }
            if let Some(health) = data.get("health").and_then(Value::as_str) {
                if let Ok(mut value) = ctx.relay_health.lock() {
                    health.clone_into(&mut value);
                }
            }
            if let Some(onion) = data.get("onion").and_then(Value::as_str) {
                if let Ok(mut value) = ctx.onion_state.lock() {
                    onion.clone_into(&mut value);
                }
            }
        }
    }
    if let Ok(mut events) = ctx.events.lock() {
        events.push_back(summary);
        while events.len() > MAX_EVENTS {
            events.pop_front();
        }
    }
    if let Ok(mut logs) = ctx.logs.lock() {
        logs.push_back(line.to_string());
        while logs.len() > MAX_EVENTS {
            logs.pop_front();
        }
    }
    if let Some(counter) = match event {
        "message_queued" => Some(&ctx.message_count),
        "attachment_queued" => Some(&ctx.attachment_count),
        "radio_burst" => Some(&ctx.radio_count),
        _ => None,
    } {
        counter.fetch_add(1, Ordering::Relaxed);
    }
    if event == "peer_ready" {
        ctx.ready_peers.fetch_add(1, Ordering::Relaxed);
    }
}

fn mark_incident(ctx: &UiContext) {
    ctx.incident.store(true, Ordering::Release);
    let ts_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(value) => value.as_millis(),
        Err(_) => 0,
    };
    if let Ok(root) = ctx.run_root.lock() {
        if let Some(root) = root.as_deref() {
            let incidents = root.join("incidents");
            let _ = std::fs::create_dir_all(&incidents);
            let marker = incidents.join(format!("incident-{ts_ms}.json"));
            let payload = serde_json::json!({
                "tsMs": ts_ms,
                "source": "tui",
                "runId": ctx.run_id.lock().ok().and_then(|id| id.clone()),
                "lastEvent": ctx.last_event.lock().ok().map(|event| event.clone()),
            });
            let _ = std::fs::write(marker, serde_json::to_vec_pretty(&payload).unwrap_or_default());
        }
    }
    publish_event("incident_marked", &serde_json::json!({"source":"tui", "tsMs": ts_ms}));
}

fn event_phase(event: &str) -> &'static str {
    match event {
        "run_started" => "starting",
        "android_preflight_started" | "android_ready" => "preflight",
        "peer_ready" | "pairing_completed" => "pairing",
        "message_queued" | "attachment_queued" | "radio_burst" => "workload",
        "run_completed" => "completed",
        "run_cancelled" => "cancelled",
        value if value.contains("fault") => "fault recovery",
        _ => "running",
    }
}

fn compact(value: &Value) -> String {
    let text = value.to_string();
    if text.len() <= 100 {
        text
    } else {
        let mut end = 100;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &text[..end])
    }
}

pub(crate) fn run(cli: Cli) -> Result<(), String> {
    let ctx = Arc::new(UiContext {
        cancel: AtomicBool::new(false),
        paused: AtomicBool::new(false),
        incident: AtomicBool::new(false),
        retry: AtomicBool::new(false),
        started: Instant::now(),
        run_id: Mutex::new(None),
        phase: Mutex::new("starting".to_owned()),
        last_event: Mutex::new(String::new()),
        android_status: Mutex::new("not selected".to_owned()),
        relay_endpoint: Mutex::new("pending".to_owned()),
        relay_health: Mutex::new("starting".to_owned()),
        onion_state: Mutex::new("unknown".to_owned()),
        events: Mutex::new(VecDeque::new()),
        logs: Mutex::new(VecDeque::new()),
        run_root: Mutex::new(None),
        message_count: std::sync::atomic::AtomicUsize::new(0),
        attachment_count: std::sync::atomic::AtomicUsize::new(0),
        radio_count: std::sync::atomic::AtomicUsize::new(0),
        ready_peers: std::sync::atomic::AtomicUsize::new(0),
    });
    let slot = ACTIVE.get_or_init(|| Mutex::new(None));
    *slot.lock().map_err(|_| "TUI state lock poisoned".to_owned())? = Some(ctx.clone());

    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let worker_cli = cli.clone();
    let worker = thread::spawn(move || {
        let result = crate::run_scenario(worker_cli);
        let _ = result_tx.send(result);
    });

    let mut terminal = match setup_terminal() {
        Ok(terminal) => terminal,
        Err(error) => {
            ctx.cancel.store(true, Ordering::Release);
            let _ = worker.join();
            *slot.lock().map_err(|_| "TUI state lock poisoned".to_owned())? = None;
            return Err(format!("setup TUI: {error}"));
        }
    };
    let mut show_logs = false;
    let mut scroll = 0usize;
    let loop_result: Result<Result<(), String>, String> = (|| {
        loop {
            if let Ok(result) = result_rx.try_recv() {
                break Ok(result);
            }
            if event::poll(Duration::from_millis(100))
                .map_err(|error| format!("read TUI input: {error}"))?
            {
                if let Event::Key(key) =
                    event::read().map_err(|error| format!("read TUI key: {error}"))?
                {
                    match key {
                        KeyEvent { code: KeyCode::Char('q'), .. }
                        | KeyEvent { code: KeyCode::Esc, .. } => {
                            ctx.cancel.store(true, Ordering::Release);
                        }
                        KeyEvent { code: KeyCode::Char('p'), .. }
                        | KeyEvent { code: KeyCode::Char(' '), .. } => {
                            let paused = !ctx.paused.load(Ordering::Acquire);
                            ctx.paused.store(paused, Ordering::Release);
                            publish_event(
                                if paused { "tui_paused" } else { "tui_resumed" },
                                &Value::Null,
                            );
                        }
                        KeyEvent { code: KeyCode::Char('m'), .. } => {
                            mark_incident(&ctx);
                        }
                        KeyEvent { code: KeyCode::Char('r'), .. } => {
                            ctx.retry.store(true, Ordering::Release);
                            publish_event(
                                "preflight_retry_requested",
                                &serde_json::json!({"source":"tui"}),
                            );
                        }
                        KeyEvent { code: KeyCode::Char('l'), .. } => {
                            show_logs = !show_logs;
                            scroll = 0;
                        }
                        KeyEvent { code: KeyCode::Up, .. } if show_logs => {
                            scroll = scroll.saturating_add(1);
                        }
                        KeyEvent { code: KeyCode::Down, .. } if show_logs => {
                            scroll = scroll.saturating_sub(1);
                        }
                        KeyEvent { code: KeyCode::Char('c'), modifiers, .. }
                            if modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            ctx.cancel.store(true, Ordering::Release);
                        }
                        _ => {}
                    }
                }
            }
            terminal
                .draw(|frame| draw(frame, &ctx, &cli, show_logs, scroll))
                .map_err(|error| format!("draw TUI: {error}"))?;
        }
    })();
    let _ = worker.join();
    let teardown =
        teardown_terminal(&mut terminal).map_err(|error| format!("teardown TUI: {error}"));
    *slot.lock().map_err(|_| "TUI state lock poisoned".to_owned())? = None;
    let result = loop_result?;
    teardown?;
    result
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

fn draw(frame: &mut ratatui::Frame, ctx: &UiContext, cli: &Cli, show_logs: bool, scroll: usize) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(Color::Black)), area);
    if show_logs {
        draw_logs(frame, area, ctx, scroll);
        return;
    }
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8), Constraint::Length(2)])
        .split(area);
    let elapsed = ctx.started.elapsed().as_secs();
    let phase =
        ctx.phase.lock().map(|value| value.clone()).unwrap_or_else(|_| "unknown".to_owned());
    let run_id = ctx
        .run_id
        .lock()
        .ok()
        .and_then(|value| value.clone())
        .unwrap_or_else(|| "pending".to_owned());
    let status = if ctx.cancel.load(Ordering::Acquire) {
        "CANCELLING"
    } else if ctx.paused.load(Ordering::Acquire) {
        "PAUSED"
    } else {
        "RUNNING"
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" TORCA SOAK ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(format!(
            " {status}  phase={phase}  run={run_id}  peers={}  elapsed={elapsed}s/{duration}s",
            cli.fake_peers,
            duration = cli.duration_seconds
        )),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Session"));
    frame.render_widget(header, vertical[0]);

    let columns = if area.width < 100 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(5), Constraint::Length(4)])
            .split(vertical[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(vertical[1])
    };
    let devices = vec![
        ListItem::new(format!(
            "Android: {} [{}]",
            cli.android.as_deref().unwrap_or("none"),
            ctx.android_status.lock().map(|value| value.clone()).unwrap_or_default()
        )),
        ListItem::new(format!(
            "Fake peers: {}/{} ready",
            ctx.ready_peers.load(Ordering::Relaxed),
            cli.fake_peers + usize::from(cli.android.is_some())
        )),
        ListItem::new(format!(
            "Relay: {:?} [{}]",
            cli.relay,
            ctx.relay_health.lock().map(|value| value.clone()).unwrap_or_default()
        )),
        ListItem::new(format!(
            "Onion: {}",
            ctx.onion_state.lock().map(|value| value.clone()).unwrap_or_default()
        )),
        ListItem::new(format!(
            "Endpoint: {}",
            ctx.relay_endpoint.lock().map(|value| value.clone()).unwrap_or_default()
        )),
    ];
    frame.render_widget(
        List::new(devices).block(Block::default().borders(Borders::ALL).title("Devices")),
        columns[0],
    );
    let counts = format!(
        "counters: msg={} attach={} radio={}",
        ctx.message_count.load(Ordering::Relaxed),
        ctx.attachment_count.load(Ordering::Relaxed),
        ctx.radio_count.load(Ordering::Relaxed)
    );
    let mut activity = vec![ListItem::new(counts)];
    let events = ctx
        .events
        .lock()
        .map(|events| events.iter().rev().take(8).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    activity.extend(events.into_iter().map(ListItem::new));
    let workload =
        List::new(activity).block(Block::default().borders(Borders::ALL).title("Recent activity"));
    frame.render_widget(workload, columns[1]);
    let progress = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Controls"))
        .gauge_style(Style::default().fg(if ctx.incident.load(Ordering::Acquire) {
            Color::Yellow
        } else {
            Color::Green
        }))
        .ratio(if ctx.paused.load(Ordering::Acquire) { 0.0 } else { 1.0 })
        .label("p pause  r retry  m mark  l logs  q quit");
    frame.render_widget(progress, columns[2]);
    frame.render_widget(
        Paragraph::new("TUI is observational; q requests controlled cancellation and cleanup.")
            .style(Style::default().fg(Color::DarkGray)),
        vertical[2],
    );
}

fn draw_logs(frame: &mut ratatui::Frame, area: Rect, ctx: &UiContext, scroll: usize) {
    let mut lines = ctx
        .logs
        .lock()
        .map(|events| events.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    lines.reverse();
    let items = lines.into_iter().skip(scroll).map(ListItem::new).collect::<Vec<_>>();
    frame.render_widget(Clear, area);
    frame.render_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Timeline log — l back")),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::{compact, event_phase};
    use serde_json::json;

    #[test]
    fn compact_keeps_json_utf8_safe_and_bounded() {
        let value = json!("żółć".repeat(80));
        let rendered = compact(&value);
        assert!(rendered.len() <= 103);
        assert!(rendered.is_char_boundary(rendered.len()));
    }

    #[test]
    fn event_phase_groups_workload_and_recovery_events() {
        assert_eq!(event_phase("run_started"), "starting");
        assert_eq!(event_phase("message_queued"), "workload");
        assert_eq!(event_phase("relay_fault_recovered"), "fault recovery");
        assert_eq!(event_phase("unknown_event"), "running");
    }
}
