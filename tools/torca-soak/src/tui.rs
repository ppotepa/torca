//! Interactive terminal dashboard for the soak runner.

use std::collections::VecDeque;
use std::io::{self, Stdout};
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
    events: Mutex<VecDeque<String>>,
}

static ACTIVE: OnceLock<Mutex<Option<Arc<UiContext>>>> = OnceLock::new();

fn active() -> Option<Arc<UiContext>> {
    ACTIVE.get()?.lock().ok()?.clone()
}

pub(crate) fn cancel_requested() -> bool {
    active().is_some_and(|ctx| ctx.cancel.load(Ordering::Acquire))
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
    if let Ok(mut events) = ctx.events.lock() {
        events.push_back(summary);
        while events.len() > MAX_EVENTS {
            events.pop_front();
        }
    }
}

fn compact(value: &Value) -> String {
    let text = value.to_string();
    if text.len() > 100 { format!("{}…", &text[..100]) } else { text }
}

pub(crate) fn run(cli: Cli) -> Result<(), String> {
    let ctx = Arc::new(UiContext {
        cancel: AtomicBool::new(false),
        paused: AtomicBool::new(false),
        incident: AtomicBool::new(false),
        events: Mutex::new(VecDeque::new()),
    });
    let slot = ACTIVE.get_or_init(|| Mutex::new(None));
    *slot.lock().map_err(|_| "TUI state lock poisoned".to_owned())? = Some(ctx.clone());

    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let worker_cli = cli.clone();
    let worker = thread::spawn(move || {
        let result = crate::run_scenario(worker_cli);
        let _ = result_tx.send(result);
    });

    let mut terminal = setup_terminal().map_err(|error| format!("setup TUI: {error}"))?;
    let mut show_logs = false;
    let mut scroll = 0usize;
    let result = loop {
        if let Ok(result) = result_rx.try_recv() {
            break result;
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
                        ctx.incident.store(true, Ordering::Release);
                        publish_event("incident_marked", &serde_json::json!({"source":"tui"}));
                    }
                    KeyEvent { code: KeyCode::Char('r'), .. } => {
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
    };
    let _ = worker.join();
    teardown_terminal(&mut terminal).map_err(|error| format!("teardown TUI: {error}"))?;
    *slot.lock().map_err(|_| "TUI state lock poisoned".to_owned())? = None;
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
    let elapsed = ctx.events.lock().map(|e| e.len()).unwrap_or_default();
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
            " {status}  peers={} duration={}s events={elapsed}",
            cli.fake_peers, cli.duration_seconds
        )),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Session"));
    frame.render_widget(header, vertical[0]);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(vertical[1]);
    let devices = vec![
        ListItem::new(format!("Android: {}", cli.android.as_deref().unwrap_or("none"))),
        ListItem::new(format!("Fake peers: {}", cli.fake_peers)),
        ListItem::new(format!("Relay: {:?}", cli.relay)),
    ];
    frame.render_widget(
        List::new(devices).block(Block::default().borders(Borders::ALL).title("Devices")),
        columns[0],
    );
    let events = ctx
        .events
        .lock()
        .map(|events| events.iter().rev().take(8).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let workload = List::new(events.into_iter().map(ListItem::new).collect::<Vec<_>>())
        .block(Block::default().borders(Borders::ALL).title("Recent activity"));
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
        .events
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
