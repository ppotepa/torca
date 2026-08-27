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

use crate::domain::{Configuration, DeployAction, DeployPlan};

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

const ACTIONS: [(&str, Option<DeployAction>); 7] = [
    ("Run installed clients", Some(DeployAction::RunInstalled)),
    ("Redeploy current artifacts", Some(DeployAction::RedeployCurrent)),
    ("Rebuild clients and relay", Some(DeployAction::Rebuild)),
    ("Full redeploy", Some(DeployAction::FullRedeploy)),
    ("Provider maintenance", Some(DeployAction::ProviderMaintenance)),
    ("Collect logs", Some(DeployAction::CollectLogs)),
    ("Resume interrupted deployment", None),
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

/// Run the action selector and enter the contextual options screen.
fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    theme_kind: ThemeKind,
    no_color: bool,
) -> io::Result<Option<WizardSelection>> {
    let mut theme_kind = theme_kind;
    let mut no_color = no_color;
    let mut theme = if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
    persist_ui_config(theme_kind, no_color);
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
                    .style(Style::default().fg(theme.accent).bg(theme.background))
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
            let action_area = crate::tui::layout::columns(layout[1]);
            frame.render_widget(
                List::new(entries)
                    .style(Style::default().fg(theme.text).bg(theme.panel))
                    .block(Block::default().title("Select action").borders(Borders::ALL)),
                action_area[0],
            );
            if action_area.len() >= 2 {
                let details = ACTIONS[selected].1.map_or_else(
                    || "Resume the most recent checkpoint.".to_owned(),
                    crate::tui::screens::action::details,
                );
                frame.render_widget(
                    Paragraph::new(details)
                        .style(Style::default().fg(theme.text).bg(theme.panel))
                        .block(Block::default().title("Selected action").borders(Borders::ALL)),
                    action_area[1],
                );
            }
            if action_area.len() >= 3 {
                let context = ACTIONS[selected].1.map_or_else(
                    || "Checkpoint recovery\nCompleted stages are reused when the plan fingerprint matches.".to_owned(),
                    |action| {
                        let plan = DeployPlan::normal(
                            action,
                            crate::planner::all_client_targets(),
                            Configuration::Debug,
                        );
                        let descriptor = plan.communication_provider.descriptor();
                        format!(
                            "Provider\n{}\n{}\n\nWarm-up\n{}\n\nWarnings\n{}",
                            descriptor.label,
                            descriptor.description,
                            descriptor.warmup_stages.join("\n"),
                            if plan.capabilities().destructive {
                                "destructive operation"
                            } else {
                                "none"
                            },
                        )
                    },
                );
                frame.render_widget(
                    Paragraph::new(context)
                        .style(Style::default().fg(theme.text).bg(theme.panel))
                        .block(Block::default().title("Context").borders(Borders::ALL)),
                    action_area[2],
                );
            }
            frame.render_widget(
                Paragraph::new("Up/Down select   Enter continue   t theme   c color   q quit")
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL)),
                layout[2],
            );
        })?;
        if let Some(key) = input.read()? {
            match key {
                KeyCode::Char('t') => {
                    theme_kind = next_theme(theme_kind);
                    theme =
                        if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
                    persist_ui_config(theme_kind, no_color);
                }
                KeyCode::Char('c') => {
                    no_color = !no_color;
                    theme =
                        if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
                    persist_ui_config(theme_kind, no_color);
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => selected = (selected + 1).min(ACTIONS.len() - 1),
                KeyCode::Enter => {
                    if selected == ACTIONS.len() - 1 {
                        return Ok(Some(WizardSelection::Resume));
                    }
                    let action = ACTIONS[selected].1.expect("action entry");
                    return Ok(screens::options::edit_plan(
                        terminal, action, theme_kind, no_color,
                    )?
                    .map(|plan| WizardSelection::Plan(plan.normalized())));
                }
                _ => {}
            }
        }
    }
}

fn next_theme(theme: ThemeKind) -> ThemeKind {
    match theme {
        ThemeKind::Aurora => ThemeKind::Amber,
        ThemeKind::Amber => ThemeKind::HighContrast,
        ThemeKind::HighContrast => ThemeKind::Aurora,
    }
}

fn persist_ui_config(theme: ThemeKind, no_color: bool) {
    let Ok(root) = std::env::current_dir() else {
        return;
    };
    let path = root.join(".torca").join("deploy").join("ui.json");
    let Some(parent) = path.parent() else {
        return;
    };
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
