//! Interactive soak-plan editor. The wizard only selects a plan; execution
//! remains owned by the scenario backends in `main`.

use std::io;
use std::path::PathBuf;
use std::process::Command;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::{Cli, CommunicationProvider, FaultProfile, FixtureMode, Scenario, Workload};

const SCENARIOS: [(Scenario, &str, &str); 5] = [
    (
        Scenario::ActiveMessaging,
        "Active messaging battery",
        "Android + five bots; contacts are created and messages flow periodically",
    ),
    (
        Scenario::IdleBattery,
        "Idle battery baseline",
        "Physical Android measurement with no synthetic contacts or traffic",
    ),
    (
        Scenario::Connectivity,
        "Connectivity recovery",
        "Repeated network loss/recovery validation on Android",
    ),
    (
        Scenario::RuntimeLab,
        "Multi-peer runtime lab",
        "Fake peers exercise messaging, attachments, radio and controlled faults",
    ),
    (
        Scenario::Deterministic,
        "Deterministic code soak",
        "Repeated Rust policy/runtime test suites without a device",
    ),
];

#[derive(Clone, Copy)]
enum Field {
    Device,
    Duration,
    Peers,
    RequireUnplugged,
    RequireScreenOff,
    NativeDiagnostics,
    Fixture,
    Provider,
}

pub(crate) fn choose_plan() -> Result<Option<Cli>, String> {
    let devices = adb_devices();
    enable_raw_mode().map_err(|error| format!("enable terminal raw mode: {error}"))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)
        .map_err(|error| format!("enter alternate screen: {error}"))?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))
        .map_err(|error| format!("create terminal: {error}"))?;
    let result = select_scenario(&mut terminal).and_then(|scenario| {
        scenario.map(|value| edit_plan(&mut terminal, value, &devices)).transpose()
    });
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
    result.map(Option::flatten).map_err(|error| format!("soak wizard: {error}"))
}

fn select_scenario(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> io::Result<Option<Scenario>> {
    let mut selected = 0usize;
    loop {
        terminal.draw(|frame| {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Length(3), Constraint::Min(10), Constraint::Length(3)])
                .split(frame.area());
            frame.render_widget(
                Paragraph::new("Torca soak")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                    .block(Block::default().borders(Borders::ALL)),
                rows[0],
            );
            let entries = SCENARIOS
                .iter()
                .enumerate()
                .map(|(index, (_, title, description))| {
                    let marker = if index == selected { ">" } else { " " };
                    ListItem::new(format!(" {marker} {} {title}\n     {description}", index + 1))
                })
                .collect::<Vec<_>>();
            frame.render_widget(
                List::new(entries)
                    .block(Block::default().title("Select scenario").borders(Borders::ALL)),
                rows[1],
            );
            frame.render_widget(
                Paragraph::new("Up/Down select   Enter configure   q quit")
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL)),
                rows[2],
            );
        })?;
        match read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(SCENARIOS.len() - 1),
            KeyCode::Enter => return Ok(Some(SCENARIOS[selected].0)),
            _ => {}
        }
    }
}

fn edit_plan(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    scenario: Scenario,
    devices: &[String],
) -> io::Result<Option<Cli>> {
    let mut field = Field::Device;
    let mut device_index = 0usize;
    let mut duration_minutes: u64 = if scenario == Scenario::Connectivity { 10 } else { 60 };
    let mut peers: usize = if scenario == Scenario::ActiveMessaging { 5 } else { 3 };
    let mut require_unplugged =
        matches!(scenario, Scenario::IdleBattery | Scenario::ActiveMessaging);
    let mut require_screen_off =
        matches!(scenario, Scenario::IdleBattery | Scenario::ActiveMessaging);
    let mut native_diagnostics = true;
    let mut fixture =
        if scenario == Scenario::ActiveMessaging { FixtureMode::Auto } else { FixtureMode::None };
    // New physical soak runs always measure the production Iroh provider.
    let mut provider = CommunicationProvider::default();
    loop {
        let device = devices.get(device_index).map_or("none detected", String::as_str);
        terminal.draw(|frame| {
            let text = format!(
                "Scenario: {}\n\n{} Android: {device}\n{} Provider: {provider:?}\n{} Duration: {duration_minutes} min\n{} Fake peers: {peers}\n{} Fixture: {fixture:?}\n{} Require unplugged: {require_unplugged}\n{} Require screen off: {require_screen_off}\n{} Native diagnostics: {native_diagnostics}\n\nLeft/Right change   Tab/Up/Down field   Enter start   Esc back",
                scenario_label(scenario),
                marker(matches!(field, Field::Device)),
                marker(matches!(field, Field::Provider)),
                marker(matches!(field, Field::Duration)),
                marker(matches!(field, Field::Peers)),
                marker(matches!(field, Field::Fixture)),
                marker(matches!(field, Field::RequireUnplugged)),
                marker(matches!(field, Field::RequireScreenOff)),
                marker(matches!(field, Field::NativeDiagnostics)),
            );
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().title("Soak plan").borders(Borders::ALL),
                ),
                frame.area(),
            );
        })?;
        match read_key()? {
            KeyCode::Esc => return Ok(None),
            KeyCode::Tab | KeyCode::Down => field = next_field(field),
            KeyCode::Up => field = previous_field(field),
            key @ (KeyCode::Left | KeyCode::Right) => {
                let increase = key == KeyCode::Right;
                match field {
                    Field::Device if !devices.is_empty() => {
                        device_index = if increase {
                            (device_index + 1) % devices.len()
                        } else if device_index == 0 {
                            devices.len() - 1
                        } else {
                            device_index - 1
                        };
                    }
                    Field::Duration => {
                        duration_minutes = if increase {
                            (duration_minutes + 5).min(24 * 60)
                        } else {
                            duration_minutes.saturating_sub(5).max(1)
                        };
                    }
                    Field::Peers => {
                        peers = if increase {
                            (peers + 1).min(20)
                        } else {
                            peers.saturating_sub(1).max(1)
                        };
                    }
                    Field::Fixture => {
                        fixture = match (fixture, increase) {
                            (FixtureMode::Auto, true) => FixtureMode::Provision,
                            (FixtureMode::None, true) => FixtureMode::Auto,
                            (FixtureMode::Provision, true) => FixtureMode::Reuse,
                            (FixtureMode::Reuse, true) => FixtureMode::Auto,
                            (FixtureMode::Auto, false) => FixtureMode::Reuse,
                            (FixtureMode::Provision, false) => FixtureMode::None,
                            (FixtureMode::None, false) => FixtureMode::Provision,
                            (FixtureMode::Reuse, false) => FixtureMode::Auto,
                        };
                    }
                    Field::Provider => {
                        let providers = CommunicationProvider::selectable();
                        let current =
                            providers.iter().position(|value| *value == provider).unwrap_or(0);
                        let next = if increase {
                            (current + 1) % providers.len()
                        } else if current == 0 {
                            providers.len() - 1
                        } else {
                            current - 1
                        };
                        provider = providers[next].clone();
                    }
                    Field::RequireUnplugged => require_unplugged = !require_unplugged,
                    Field::RequireScreenOff => require_screen_off = !require_screen_off,
                    Field::NativeDiagnostics => native_diagnostics = !native_diagnostics,
                    Field::Device => {}
                }
            }
            KeyCode::Enter => {
                let android = matches!(
                    scenario,
                    Scenario::ActiveMessaging | Scenario::IdleBattery | Scenario::Connectivity
                )
                .then(|| devices.get(device_index).cloned())
                .flatten();
                if matches!(
                    scenario,
                    Scenario::ActiveMessaging | Scenario::IdleBattery | Scenario::Connectivity
                ) && android.is_none()
                {
                    continue;
                }
                let plan = Cli {
                    scenario,
                    android,
                    android_auto_deploy: matches!(scenario, Scenario::ActiveMessaging),
                    preserve_profiles: false,
                    fake_peers: peers,
                    duration_seconds: duration_minutes * 60,
                    communication_provider: provider.clone(),
                    workload: Workload::Balanced,
                    radio: false,
                    fault_profile: if scenario == Scenario::ActiveMessaging {
                        FaultProfile::None
                    } else {
                        FaultProfile::Controlled
                    },
                    fixture,
                    fixture_name: "android-default".into(),
                    output: PathBuf::from(".torca/soak"),
                    lab_peer: None,
                    bot_host: None,
                    bot_token: None,
                    repo_root: PathBuf::from("."),
                    plain: false,
                    tui: true,
                    require_unplugged,
                    require_screen_off,
                    collect_native_diagnostics: native_diagnostics,
                    validate_after: true,
                    iterations: 20,
                };
                if review_plan(terminal, &plan)? {
                    return Ok(Some(plan));
                }
            }
            _ => {}
        }
    }
}

fn review_plan(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    plan: &Cli,
) -> io::Result<bool> {
    loop {
        terminal.draw(|frame| {
            let android = plan.android.as_deref().unwrap_or("not required");
            let text = format!(
                "Scenario: {}\nAndroid: {android}\nProvider: {:?}\nDuration: {} min\nFake peers: {}\nFixture: {:?}\nAuto deploy: {}\nUnplugged required: {}\nScreen off required: {}\nNative diagnostics: {}\nValidation: {}\n\nEnter starts the soak. Esc returns to configuration.",
                scenario_label(plan.scenario),
                plan.communication_provider,
                plan.duration_seconds.div_ceil(60),
                plan.fake_peers,
                plan.fixture,
                plan.android_auto_deploy,
                plan.require_unplugged,
                plan.require_screen_off,
                plan.collect_native_diagnostics,
                plan.validate_after,
            );
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().title("Review soak plan").borders(Borders::ALL),
                ),
                frame.area(),
            );
        })?;
        match read_key()? {
            KeyCode::Enter => return Ok(true),
            KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
            _ => {}
        }
    }
}

fn read_key() -> io::Result<KeyCode> {
    loop {
        if let Event::Key(key) = event::read()?
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            return Ok(key.code);
        }
    }
}

fn adb_devices() -> Vec<String> {
    Command::new("adb")
        .arg("devices")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            (fields.next() == Some("device")).then(|| serial.to_owned())
        })
        .collect()
}

fn marker(active: bool) -> &'static str {
    if active { ">" } else { " " }
}

fn scenario_label(scenario: Scenario) -> &'static str {
    SCENARIOS
        .iter()
        .find_map(|(value, label, _)| (*value == scenario).then_some(*label))
        .unwrap_or("Unknown")
}

fn next_field(field: Field) -> Field {
    match field {
        Field::Device => Field::Duration,
        Field::Duration => Field::Peers,
        Field::Peers => Field::Fixture,
        Field::Fixture => Field::Provider,
        Field::Provider => Field::RequireUnplugged,
        Field::RequireUnplugged => Field::RequireScreenOff,
        Field::RequireScreenOff => Field::NativeDiagnostics,
        Field::NativeDiagnostics => Field::Device,
    }
}

fn previous_field(field: Field) -> Field {
    match field {
        Field::Device => Field::NativeDiagnostics,
        Field::Duration => Field::Device,
        Field::Peers => Field::Duration,
        Field::Fixture => Field::Peers,
        Field::Provider => Field::Fixture,
        Field::RequireUnplugged => Field::Provider,
        Field::RequireScreenOff => Field::RequireUnplugged,
        Field::NativeDiagnostics => Field::RequireScreenOff,
    }
}
