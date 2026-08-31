use crossterm::{
    event::KeyCode,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::io;

use crate::domain::{DeployAction, DeployPlan};
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

const ACTIONS: [(&str, DeployAction); 5] = [
    ("Run installed clients", DeployAction::RunInstalled),
    ("Redeploy current artifacts", DeployAction::RedeployCurrent),
    ("Rebuild clients", DeployAction::Rebuild),
    ("Full redeploy", DeployAction::FullRedeploy),
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
    let last_plan = load_last_plan().map(DeployPlan::normalized);
    let mut action_selected = last_plan
        .as_ref()
        .and_then(|plan| ACTIONS.iter().position(|(_, action)| *action == plan.action))
        .unwrap_or(0);
    let mut input = InputGuard::default();
    loop {
        terminal.draw(|frame| render_action(frame, action_selected, last_plan.as_ref(), theme))?;
        let Some(key) = input.read()? else { continue };
        match key {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            KeyCode::Char('l') => {
                if let Some(plan) = last_plan.clone() {
                    return Ok(Some(WizardSelection::Plan(plan.normalized())));
                }
            }
            KeyCode::Char('r') => {
                if has_resumable_run() {
                    return Ok(Some(WizardSelection::Resume));
                }
            }
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
                let plan = plan_for_action(last_plan.as_ref(), action);
                if let Some(plan) =
                    screens::options::edit_plan(terminal, plan, theme_kind, no_color)?
                {
                    let plan = plan.normalized();
                    save_last_plan(&plan)?;
                    return Ok(Some(WizardSelection::Plan(plan)));
                }
            }
            _ => {}
        }
    }
}

fn render_action(
    frame: &mut ratatui::Frame<'_>,
    selected: usize,
    last_plan: Option<&DeployPlan>,
    theme: Theme,
) {
    frame
        .render_widget(Block::default().style(Style::default().bg(theme.background)), frame.area());
    let area = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Length(3), Constraint::Min(8), Constraint::Length(3)])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "TORCA DEPLOY",
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  /  IROH DEPLOYMENT", Style::default().fg(theme.info)),
        ]))
        .style(Style::default().bg(theme.background))
        .alignment(Alignment::Center)
        .block(
            Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.accent)),
        ),
        area[0],
    );
    let columns = crate::tui::layout::columns(area[1]);
    let items: Vec<ListItem> = ACTIONS
        .iter()
        .enumerate()
        .map(|(index, (label, _action))| {
            let active = index == selected;
            ListItem::new(Line::from(vec![
                Span::styled(
                    if active { "> " } else { "  " },
                    Style::default().fg(if active { theme.selected } else { theme.muted }),
                ),
                Span::styled(
                    format!("{} ", index + 1),
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    *label,
                    Style::default()
                        .fg(if active { theme.selected } else { theme.text })
                        .add_modifier(if active { Modifier::BOLD } else { Modifier::empty() }),
                ),
            ]))
        })
        .collect();
    frame.render_widget(
        List::new(items).style(Style::default().fg(theme.text).bg(theme.panel)).block(
            Block::default()
                .title(" 1. Choose action ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border)),
        ),
        columns[0],
    );
    if columns.len() > 1 {
        frame.render_widget(
            Paragraph::new(last_plan_lines(last_plan, theme))
                .style(Style::default().bg(theme.panel))
                .block(
                    Block::default()
                        .title(" 2. Last configuration ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.info)),
                ),
            columns[1],
        );
    }
    if columns.len() > 2 {
        let action = ACTIONS[selected].1;
        frame.render_widget(
            Paragraph::new(action_lines(action, theme))
                .style(Style::default().bg(theme.panel))
                .block(
                    Block::default()
                        .title(" 3. Review flow ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.success)),
                ),
            columns[2],
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Up/Down", Style::default().fg(theme.info).add_modifier(Modifier::BOLD)),
            Span::raw(" select   "),
            Span::styled("Enter", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
            Span::raw(" configure   "),
            Span::styled("L", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
            Span::raw(" deploy last   R resume   Esc quit"),
        ]))
        .style(Style::default().fg(theme.text).bg(theme.background))
        .alignment(Alignment::Center)
        .block(
            Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.border)),
        ),
        area[2],
    );
}

fn last_plan_lines(plan: Option<&DeployPlan>, theme: Theme) -> Vec<Line<'static>> {
    let Some(plan) = plan else {
        return vec![
            Line::from(Span::styled(
                "No previous deployment yet.",
                Style::default().fg(theme.muted),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Choose an action to create the first plan.",
                Style::default().fg(theme.info),
            )),
        ];
    };
    let targets = plan.targets.iter().map(ToString::to_string).collect::<Vec<_>>().join(" + ");
    let rows = [
        ("Action", plan.action.to_string(), theme.accent),
        ("Targets", targets, theme.info),
        ("Build", plan.configuration.to_string(), theme.warning),
        ("Data", format!("{:?}", plan.client_data), theme.danger),
        ("Validation", format!("{:?}", plan.validation), theme.success),
        ("Launch", format!("{:?}", plan.launch), theme.selected),
        (
            "Iroh profile",
            plan.provider_profile.clone().unwrap_or_else(|| "default".into()),
            theme.accent,
        ),
    ];
    rows.into_iter()
        .map(|(label, value, color)| {
            Line::from(vec![
                Span::styled(format!("{label:<13}"), Style::default().fg(theme.muted)),
                Span::styled(value, Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ])
        })
        .collect()
}

fn action_lines(action: DeployAction, theme: Theme) -> Vec<Line<'static>> {
    let plan = DeployPlan::normal(
        action,
        crate::planner::all_client_targets(),
        crate::domain::Configuration::Debug,
    );
    let mut lines = vec![
        Line::from(Span::styled(
            action.to_string(),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for step in plan.planned_steps() {
        let (marker, color) = match step.disposition {
            crate::domain::StepDisposition::Execute => ("[run]", theme.success),
            crate::domain::StepDisposition::Reuse => ("[use]", theme.info),
            crate::domain::StepDisposition::Skip => ("[skip]", theme.muted),
            crate::domain::StepDisposition::Blocked => ("[stop]", theme.danger),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker} "), Style::default().fg(color)),
            Span::styled(step.label, Style::default().fg(theme.text)),
        ]));
    }
    lines
}

fn plan_for_action(last_plan: Option<&DeployPlan>, action: DeployAction) -> DeployPlan {
    let plan = last_plan.cloned().unwrap_or_else(|| {
        DeployPlan::normal(
            action,
            crate::planner::all_client_targets(),
            crate::domain::Configuration::Debug,
        )
    });
    let mut model = model::WizardModel::new(plan);
    model.set_action(action);
    model.plan
}

fn current_theme(kind: ThemeKind, no_color: bool) -> Theme {
    if no_color { Theme::monochrome() } else { Theme::for_kind(kind) }
}

fn load_last_plan() -> Option<DeployPlan> {
    let paths = DeployPaths::discover().ok()?;
    StateStore::new(paths).load_last_plan().ok().flatten()
}

fn save_last_plan(plan: &DeployPlan) -> io::Result<()> {
    let paths = DeployPaths::discover().map_err(io::Error::other)?;
    StateStore::new(paths).save_last_plan(plan).map_err(io::Error::other)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Configuration, Target};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn last_plan_summary_contains_the_configuration_that_will_be_reused() {
        let plan = DeployPlan::normal(
            DeployAction::FullRedeploy,
            vec![Target::Android],
            Configuration::Release,
        );
        let rendered = last_plan_lines(Some(&plan), Theme::high_contrast())
            .into_iter()
            .flat_map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .chain(std::iter::once("\n".to_owned()))
            })
            .collect::<String>();
        assert!(rendered.contains("full redeploy"));
        assert!(rendered.contains("android"));
        assert!(rendered.contains("release"));
    }

    #[test]
    fn action_screen_renders_last_configuration_at_wide_terminal_width() {
        let plan = DeployPlan::normal(
            DeployAction::Rebuild,
            vec![Target::Windows, Target::Android],
            Configuration::Debug,
        );
        let backend = TestBackend::new(140, 36);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render_action(frame, 2, Some(&plan), Theme::aurora()))
            .expect("render action screen");
        let buffer = terminal.backend().buffer();
        let rendered = buffer.content.iter().map(|cell| cell.symbol()).collect::<String>();
        assert!(rendered.contains("Last configuration"));
        assert!(rendered.contains("windows + android"));
        assert!(rendered.contains("Review flow"));
    }

    #[test]
    fn selecting_an_action_keeps_reusable_values_from_the_last_plan() {
        let previous = DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Android],
            Configuration::Release,
        );

        let next = plan_for_action(Some(&previous), DeployAction::CollectLogs);

        assert_eq!(next.action, DeployAction::CollectLogs);
        assert_eq!(next.targets, vec![Target::Android]);
        assert_eq!(next.configuration, Configuration::Release);
    }
}
