use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::domain::{
    BuildPolicy, ClientDataPolicy, CommunicationProvider, Configuration, DeployAction, DeployPlan,
    LaunchPolicy, PrivacyPolicy, ProviderMaintenancePolicy, Target, ValidationLevel,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WizardSelection {
    Plan(DeployPlan),
    Resume,
}

const ACTIONS: [(&str, Option<DeployAction>); 7] = [
    ("Run installed clients", Some(DeployAction::RunInstalled)),
    ("Redeploy current artifacts", Some(DeployAction::RedeployCurrent)),
    ("Rebuild clients and relay", Some(DeployAction::Rebuild)),
    ("Full redeploy", Some(DeployAction::FullRedeploy)),
    ("Provider maintenance", Some(DeployAction::ProviderMaintenance)),
    ("Collect logs", Some(DeployAction::CollectLogs)),
    ("Resume interrupted deployment", None),
];

pub fn choose_plan() -> io::Result<Option<WizardSelection>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run(&mut terminal);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> io::Result<Option<WizardSelection>> {
    let mut selected = 0_usize;
    let mut input = InputGuard::default();
    loop {
        terminal.draw(|frame| {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Length(3), Constraint::Min(8), Constraint::Length(3)])
                .split(frame.area());
            frame.render_widget(
                Paragraph::new("Torca deploy")
                    .style(Style::default().fg(Color::Cyan))
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL)),
                layout[0],
            );
            let entries: Vec<ListItem> = ACTIONS
                .iter()
                .enumerate()
                .map(|(index, (label, _))| {
                    let marker = if index == selected { ">" } else { " " };
                    ListItem::new(format!(" {marker} {} {label}", index + 1))
                })
                .collect();
            frame.render_widget(
                List::new(entries)
                    .block(Block::default().title("Select action").borders(Borders::ALL)),
                layout[1],
            );
            frame.render_widget(
                Paragraph::new("↑/↓ select   Enter continue   q quit")
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL)),
                layout[2],
            );
        })?;
        if let Some(key) = input.read()? {
            match key {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => selected = (selected + 1).min(ACTIONS.len() - 1),
                KeyCode::Enter => {
                    if selected == ACTIONS.len() - 1 {
                        return Ok(Some(WizardSelection::Resume));
                    }
                    let action = ACTIONS[selected].1.expect("action entry");
                    return Ok(edit_plan(terminal, action)?
                        .map(|plan| WizardSelection::Plan(plan.normalized())));
                }
                _ => {}
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Field {
    Target,
    Configuration,
    ClientData,
    ProviderMaintenance,
    Privacy,
    CommunicationProvider,
    ProviderProfile,
}

fn edit_plan(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    action: DeployAction,
) -> io::Result<Option<DeployPlan>> {
    let mut field = Field::Target;
    let mut target = 0_u8; // all, Windows, Android
    let mut configuration = Configuration::Debug;
    let mut client_data = if action == DeployAction::FullRedeploy {
        ClientDataPolicy::ResetProfile
    } else {
        ClientDataPolicy::Preserve
    };
    let mut provider_maintenance = ProviderMaintenancePolicy::Ensure;
    let mut privacy = PrivacyPolicy::Strict;
    let mut communication_provider = CommunicationProvider::Tor;
    let mut provider_profile: Option<String> = None;
    let mut input = InputGuard::default();
    loop {
        terminal.draw(|frame| {
            let target_label = match target {
                1 => "Windows",
                2 => "Android",
                _ => "All detected clients",
            };
            let data_label = match client_data {
                ClientDataPolicy::Preserve => "Preserve client data",
                ClientDataPolicy::ResetProfile => "Reset profile",
                ClientDataPolicy::ResetAll => "Reset all client data",
            };
            let provider_maintenance_label = match provider_maintenance {
                ProviderMaintenancePolicy::Ensure => "Ensure provider service",
                ProviderMaintenancePolicy::Restart => "Restart provider service, preserve identity",
                ProviderMaintenancePolicy::RepairDirectoryCache => "Repair provider local state",
                ProviderMaintenancePolicy::RotateIdentity => "Rotate provider identity (rebuild all)",
            };
            let privacy_label = privacy_label(privacy);
            let provider_label = communication_provider.protocol_label();
            let provider_profile_label = provider_profile.as_deref().unwrap_or("default");
            let provider_profile_help = if communication_provider == CommunicationProvider::Iroh {
                iroh_profile_description(provider_profile_label)
            } else {
                "provider default"
            };
            let text = format!(
                "Action: {action}\n\n{} Target: {target_label}\n{} Build: {configuration}\n{} Data: {data_label}\n{} Provider maintenance: {provider_maintenance_label}\n{} Privacy: {privacy_label}\n{} Communication protocol: {provider_label}\n{} Provider profile: {provider_profile_label}\n  {}\n\n←/→ change   Tab/↑/↓ field   Enter review   Esc back",
                marker(matches!(field, Field::Target)),
                marker(matches!(field, Field::Configuration)),
                marker(matches!(field, Field::ClientData)),
                marker(matches!(field, Field::ProviderMaintenance)),
                marker(matches!(field, Field::Privacy)),
                marker(matches!(field, Field::CommunicationProvider)),
                marker(matches!(field, Field::ProviderProfile)),
                provider_profile_help,
            );
            frame.render_widget(
                Paragraph::new(text)
                    .block(Block::default().title("Deployment options").borders(Borders::ALL)),
                frame.area(),
            );
        })?;
        if let Some(key) = input.read()? {
            match key {
                KeyCode::Esc => return Ok(None),
                KeyCode::Tab | KeyCode::Down => field = next_field(field),
                KeyCode::Up => field = previous_field(field),
                KeyCode::Left | KeyCode::Right => {
                    let direction = if key == KeyCode::Right { 1_i8 } else { -1 };
                    match field {
                        Field::Target => {
                            target = cycle_target(target, direction);
                        }
                        Field::Configuration => {
                            configuration = if configuration == Configuration::Debug {
                                Configuration::Release
                            } else {
                                Configuration::Debug
                            };
                        }
                        Field::ClientData => {
                            client_data = cycle_data(client_data, direction);
                        }
                        Field::ProviderMaintenance => {
                            provider_maintenance =
                                cycle_provider_maintenance(provider_maintenance, direction);
                        }
                        Field::Privacy => {
                            privacy = if privacy == PrivacyPolicy::Strict {
                                PrivacyPolicy::AllowCapture
                            } else {
                                PrivacyPolicy::Strict
                            };
                        }
                        Field::CommunicationProvider => {
                            communication_provider =
                                cycle_provider(communication_provider, direction);
                            if communication_provider != CommunicationProvider::Iroh {
                                provider_profile = None;
                            } else if provider_profile.is_none() {
                                provider_profile = Some("always".to_owned());
                            }
                        }
                        Field::ProviderProfile => {
                            if communication_provider == CommunicationProvider::Iroh {
                                provider_profile = Some(cycle_iroh_profile(
                                    provider_profile.as_deref().unwrap_or("always"),
                                    direction,
                                ));
                            }
                        }
                    }
                }
                KeyCode::Enter => {
                    let targets = match target {
                        1 => vec![Target::Windows],
                        2 => vec![Target::Android],
                        _ => vec![Target::Windows, Target::Android],
                    };
                    let mut plan = DeployPlan::normal(action, targets, configuration);
                    plan.client_data = client_data;
                    plan.provider_maintenance = provider_maintenance;
                    plan.privacy = privacy;
                    plan.communication_provider = communication_provider;
                    plan.provider_profile.clone_from(&provider_profile);
                    plan.client_build = if action == DeployAction::RunInstalled {
                        BuildPolicy::Reuse
                    } else {
                        plan.client_build
                    };
                    plan.provider_service_build = if action == DeployAction::ProviderMaintenance {
                        BuildPolicy::IfRequired
                    } else {
                        plan.provider_service_build
                    };
                    plan.validation = ValidationLevel::Quick;
                    plan.launch = if action == DeployAction::CollectLogs {
                        LaunchPolicy::Skip
                    } else {
                        LaunchPolicy::Restart
                    };
                    if confirm(terminal, &plan)? {
                        return Ok(Some(plan));
                    }
                }
                _ => {}
            }
        }
    }
}

fn marker(active: bool) -> &'static str {
    if active { ">" } else { " " }
}

fn iroh_profile_description(profile: &str) -> &'static str {
    match profile {
        "direct" | "direct-only" => {
            "relay/discovery off; lowest idle cost, LAN or externally reachable peers only"
        }
        "local" | "local-only" => "loopback only; simulator/lab use, no remote reachability",
        _ => "N0 relay/discovery on; recommended for mobile networks and background reachability",
    }
}

fn next_field(field: Field) -> Field {
    match field {
        Field::Target => Field::Configuration,
        Field::Configuration => Field::ClientData,
        Field::ClientData => Field::ProviderMaintenance,
        Field::ProviderMaintenance => Field::Privacy,
        Field::Privacy => Field::CommunicationProvider,
        Field::CommunicationProvider => Field::ProviderProfile,
        Field::ProviderProfile => Field::Target,
    }
}

fn previous_field(field: Field) -> Field {
    match field {
        Field::Target => Field::ProviderProfile,
        Field::Configuration => Field::Target,
        Field::ClientData => Field::Configuration,
        Field::ProviderMaintenance => Field::ClientData,
        Field::Privacy => Field::ProviderMaintenance,
        Field::CommunicationProvider => Field::Privacy,
        Field::ProviderProfile => Field::CommunicationProvider,
    }
}

fn cycle_data(current: ClientDataPolicy, direction: i8) -> ClientDataPolicy {
    let index = match current {
        ClientDataPolicy::Preserve => 0,
        ClientDataPolicy::ResetProfile => 1,
        ClientDataPolicy::ResetAll => 2,
    };
    match (index as i8 + direction).rem_euclid(3) {
        1 => ClientDataPolicy::ResetProfile,
        2 => ClientDataPolicy::ResetAll,
        _ => ClientDataPolicy::Preserve,
    }
}

fn cycle_target(current: u8, direction: i8) -> u8 {
    match (current, direction > 0) {
        (0, false) => 2,
        (1, false) => 0,
        (2, false) => 1,
        (0, true) => 1,
        (1, true) => 2,
        (2, true) => 0,
        _ => 0,
    }
}

fn cycle_provider(current: CommunicationProvider, direction: i8) -> CommunicationProvider {
    let providers = CommunicationProvider::selectable();
    let index = providers.iter().position(|provider| *provider == current).unwrap_or(0);
    let next = (index as i8 + direction).rem_euclid(providers.len() as i8) as usize;
    providers[next]
}

fn cycle_iroh_profile(current: &str, direction: i8) -> String {
    let profiles = ["always", "direct", "local"];
    let index = profiles.iter().position(|profile| *profile == current).unwrap_or(0);
    profiles[(index as i8 + direction).rem_euclid(profiles.len() as i8) as usize].to_owned()
}

fn cycle_provider_maintenance(
    current: ProviderMaintenancePolicy,
    direction: i8,
) -> ProviderMaintenancePolicy {
    let index = match current {
        ProviderMaintenancePolicy::Ensure => 0,
        ProviderMaintenancePolicy::Restart => 1,
        ProviderMaintenancePolicy::RepairDirectoryCache => 2,
        ProviderMaintenancePolicy::RotateIdentity => 3,
    };
    match (index as i8 + direction).rem_euclid(4) {
        1 => ProviderMaintenancePolicy::Restart,
        2 => ProviderMaintenancePolicy::RepairDirectoryCache,
        3 => ProviderMaintenancePolicy::RotateIdentity,
        _ => ProviderMaintenancePolicy::Ensure,
    }
}

fn privacy_label(policy: PrivacyPolicy) -> &'static str {
    match policy {
        PrivacyPolicy::Strict => "Strict (block screenshots/recording)",
        PrivacyPolicy::AllowCapture => "Allow screenshots/recording",
    }
}

/// Prevents terminal key auto-repeat from moving through several wizard
/// options in one hold. Modern terminals expose `Repeat`/`Release` directly;
/// the short fallback interval also covers terminals that report repeats as
/// ordinary `Press` events.
#[derive(Default)]
struct InputGuard {
    last_press: Option<(KeyCode, Instant)>,
}

impl InputGuard {
    fn read(&mut self) -> io::Result<Option<KeyCode>> {
        loop {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if let Some(key) = self.accept(key) {
                return Ok(Some(key));
            }
        }
    }

    fn accept(&mut self, key: KeyEvent) -> Option<KeyCode> {
        match key.kind {
            KeyEventKind::Release => {
                if self.last_press.map(|(code, _)| code) == Some(key.code) {
                    self.last_press = None;
                }
                None
            }
            KeyEventKind::Repeat => {
                // A held arrow/left/right key must not act like multiple
                // independent navigation commands.
                None
            }
            KeyEventKind::Press => {
                let now = Instant::now();
                let navigation = matches!(
                    key.code,
                    KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Tab
                );
                if navigation
                    && self.last_press.is_some_and(|(code, at)| {
                        code == key.code && now.duration_since(at) < Duration::from_millis(220)
                    })
                {
                    return None;
                }
                self.last_press = Some((key.code, now));
                Some(key.code)
            }
        }
    }
}

fn confirm(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    plan: &DeployPlan,
) -> io::Result<bool> {
    let mut input = InputGuard::default();
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let text = format!(
                "Action: {}\nTargets: Windows, Android\nBuild: {}\nCommunication protocol: {}\nProvider profile: {:?}\n  {}\nProvider maintenance: {:?}\nPrivacy: {}\n\nPress y to execute, n/Esc to cancel",
                plan.action,
                plan.configuration,
                plan.communication_provider.protocol_label(),
                plan.provider_profile.as_deref().unwrap_or("default"),
                if plan.communication_provider == CommunicationProvider::Iroh {
                    iroh_profile_description(plan.provider_profile.as_deref().unwrap_or("always"))
                } else {
                    "provider default"
                },
                plan.provider_maintenance,
                privacy_label(plan.privacy)
            );
            frame.render_widget(
                Paragraph::new(text)
                    .block(Block::default().title("Confirm deployment").borders(Borders::ALL)),
                area,
            );
        })?;
        if let Some(key) = input.read()? {
            match key {
                KeyCode::Char('y') | KeyCode::Enter => return Ok(true),
                KeyCode::Char('n') | KeyCode::Esc => return Ok(false),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn held_navigation_repeat_is_ignored() {
        let mut guard = InputGuard::default();
        let press = KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Press);
        let repeat =
            KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Repeat);
        assert_eq!(guard.accept(press), Some(KeyCode::Down));
        assert_eq!(guard.accept(repeat), None);
    }
}
