use crossterm::{
    event::KeyCode,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::io;

use crate::domain::{
    CommunicationProvider, DeployAction, DeployPlan, ProviderMetadataExt, iroh_provider,
};
use crate::persistence::{DeployPaths, StateStore};

pub mod app;
pub mod input;
pub mod layout;
pub mod model;
pub mod screens;
pub mod theme;
pub mod widgets;

use input::InputGuard;
use theme::{Theme, ThemeKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WizardSelection {
    Plan(DeployPlan),
    Resume,
}

const ACTIONS: [(&str, DeployAction); 6] = [
    ("Run installed clients", DeployAction::RunInstalled),
    ("Redeploy current artifacts", DeployAction::RedeployCurrent),
    ("Rebuild clients", DeployAction::Rebuild),
    ("Full redeploy", DeployAction::FullRedeploy),
    ("Provider maintenance", DeployAction::ProviderMaintenance),
    ("Collect logs", DeployAction::CollectLogs),
];

pub fn choose_plan(theme_kind: ThemeKind, no_color: bool) -> io::Result<Option<WizardSelection>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run(&mut terminal, theme_kind, no_color);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut theme_kind: ThemeKind,
    mut no_color: bool,
) -> io::Result<Option<WizardSelection>> {
    let mut theme = current_theme(theme_kind, no_color);
    persist_ui_config(theme_kind, no_color);
    let mut provider_selected = 0_usize;
    let mut action_selected = 0_usize;
    let mut input = InputGuard::default();
    loop {
        let provider = iroh_provider();
        terminal.draw(|frame| render_provider(frame, provider, provider_selected, theme))?;
        let Some(key) = input.read()? else { continue };
        match key {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            KeyCode::Char('l') => {
                if let Some(plan) = load_last_plan() {
                    return Ok(Some(WizardSelection::Plan(plan.normalized())));
                }
            }
            KeyCode::Char('r') => {
                if has_resumable_run() {
                    return Ok(Some(WizardSelection::Resume));
                }
            }
            KeyCode::Up | KeyCode::Left => provider_selected = provider_selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Right => {
                provider_selected = 0;
            }
            KeyCode::Char('t') => {
                theme_kind = next_theme(theme_kind);
                theme = current_theme(theme_kind, no_color);
                persist_ui_config(theme_kind, no_color);
            }
            KeyCode::Char('c') => {
                no_color = !no_color;
                theme = current_theme(theme_kind, no_color);
                persist_ui_config(theme_kind, no_color);
            }
            KeyCode::Enter => loop {
                let selected_provider = iroh_provider();
                terminal.draw(|frame| {
                    render_action(frame, selected_provider.clone(), action_selected, theme);
                })?;
                let Some(action_key) = input.read()? else { continue };
                match action_key {
                    KeyCode::Esc => break,
                    KeyCode::Up => action_selected = action_selected.saturating_sub(1),
                    KeyCode::Down => action_selected = (action_selected + 1).min(ACTIONS.len() - 1),
                    KeyCode::Char('t') => {
                        theme_kind = next_theme(theme_kind);
                        theme = current_theme(theme_kind, no_color);
                        persist_ui_config(theme_kind, no_color);
                    }
                    KeyCode::Char('c') => {
                        no_color = !no_color;
                        theme = current_theme(theme_kind, no_color);
                        persist_ui_config(theme_kind, no_color);
                    }
                    KeyCode::Enter => {
                        let action = ACTIONS[action_selected].1;
                        if action == DeployAction::ProviderMaintenance
                            && !selected_provider.descriptor().managed_service
                        {
                            continue;
                        }
                        if let Some(plan) = screens::options::edit_plan_for_provider(
                            terminal,
                            action,
                            selected_provider,
                            theme_kind,
                            no_color,
                        )? {
                            return Ok(Some(WizardSelection::Plan(plan.normalized())));
                        }
                    }
                    _ => {}
                }
            },
            _ => {}
        }
    }
}

fn render_provider(
    frame: &mut ratatui::Frame<'_>,
    provider: CommunicationProvider,
    selected: usize,
    theme: Theme,
) {
    let area = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Length(3), Constraint::Min(8), Constraint::Length(3)])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new("TORCA DEPLOY Â· 1 Provider")
            .style(Style::default().fg(theme.accent).bg(theme.background))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL)),
        area[0],
    );
    let columns = crate::tui::layout::columns(area[1]);
    let providers: Vec<ListItem> = [iroh_provider()]
        .iter()
        .enumerate()
        .map(|(index, item)| {
            ListItem::new(format!(
                " {} {} {}",
                if index == selected { ">" } else { " " },
                index + 1,
                item.descriptor().label
            ))
        })
        .collect();
    frame.render_widget(
        List::new(providers)
            .style(Style::default().fg(theme.text).bg(theme.panel))
            .block(Block::default().title("Choose communication provider").borders(Borders::ALL)),
        columns[0],
    );
    if columns.len() > 1 {
        let descriptor = provider.descriptor();
        let details = format!(
            "{}\n\nManaged service: {}\nEndpoint required: {}\n\nProfiles\n{}\n\nWarm-up\n{}",
            descriptor.description,
            if descriptor.managed_service { "yes" } else { "none" },
            descriptor.endpoint_required,
            descriptor
                .profiles
                .iter()
                .map(|profile| format!("{} â€” {}", profile.label, profile.description))
                .collect::<Vec<_>>()
                .join("\n"),
            descriptor.warmup_stages.join("\n")
        );
        frame.render_widget(
            Paragraph::new(details)
                .style(Style::default().fg(theme.text).bg(theme.panel))
                .block(Block::default().title("Selected provider").borders(Borders::ALL)),
            columns[1],
        );
    }
    frame.render_widget(Paragraph::new("Up/Down or Left/Right select   Enter continue   L load last   R resume   t theme   c color   q quit").alignment(Alignment::Center).block(Block::default().borders(Borders::ALL)), area[2]);
}

fn render_action(
    frame: &mut ratatui::Frame<'_>,
    provider: CommunicationProvider,
    selected: usize,
    theme: Theme,
) {
    let area = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Length(3), Constraint::Min(8), Constraint::Length(3)])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(format!(
            "TORCA DEPLOY Â· 1 Provider Â· 2 Action Â· {}",
            provider.descriptor().label
        ))
        .style(Style::default().fg(theme.accent).bg(theme.background))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL)),
        area[0],
    );
    let columns = crate::tui::layout::columns(area[1]);
    let items: Vec<ListItem> = ACTIONS
        .iter()
        .enumerate()
        .map(|(index, (label, action))| {
            let unavailable = *action == DeployAction::ProviderMaintenance
                && !provider.descriptor().managed_service;
            ListItem::new(format!(
                " {} {} {}{}",
                if index == selected { ">" } else { " " },
                index + 1,
                label,
                if unavailable { " (unavailable)" } else { "" }
            ))
        })
        .collect();
    frame.render_widget(
        List::new(items)
            .style(Style::default().fg(theme.text).bg(theme.panel))
            .block(Block::default().title("What do you want to do?").borders(Borders::ALL)),
        columns[0],
    );
    if columns.len() > 1 {
        let action = ACTIONS[selected].1;
        let detail = if action == DeployAction::ProviderMaintenance
            && !provider.descriptor().managed_service
        {
            "Unavailable: this provider has no deployer-managed service.".to_owned()
        } else {
            crate::tui::screens::action::details(action)
        };
        frame.render_widget(
            Paragraph::new(detail)
                .style(Style::default().fg(theme.text).bg(theme.panel))
                .block(Block::default().title("Selected action").borders(Borders::ALL)),
            columns[1],
        );
    }
    frame.render_widget(
        Paragraph::new("Up/Down select   Enter continue   Esc provider   t theme   c color")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL)),
        area[2],
    );
}

fn current_theme(kind: ThemeKind, no_color: bool) -> Theme {
    if no_color { Theme::monochrome() } else { Theme::for_kind(kind) }
}

fn load_last_plan() -> Option<DeployPlan> {
    let paths = DeployPaths::discover().ok()?;
    StateStore::new(paths).load_last_plan().ok().flatten()
}

fn has_resumable_run() -> bool {
    let Some(paths) = DeployPaths::discover().ok() else { return false };
    StateStore::new(paths).has_resumable_run().unwrap_or(false)
}

fn next_theme(theme: ThemeKind) -> ThemeKind {
    match theme {
        ThemeKind::Aurora => ThemeKind::Amber,
        ThemeKind::Amber => ThemeKind::HighContrast,
        ThemeKind::HighContrast => ThemeKind::Aurora,
    }
}

fn persist_ui_config(theme: ThemeKind, no_color: bool) {
    let Ok(root) = std::env::current_dir() else { return };
    let path = root.join(".torca").join("deploy").join("ui.json");
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let theme = match theme {
        ThemeKind::Aurora => "aurora",
        ThemeKind::Amber => "amber",
        ThemeKind::HighContrast => "high-contrast",
    };
    let contents = format!("{{\n  \"theme\": \"{theme}\",\n  \"noColor\": {no_color}\n}}\n");
    let _ = std::fs::write(path, contents);
}
