use std::io;

use crossterm::event::KeyCode;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::domain::{ClientDataPolicy, DeployPlan, FieldAvailability, FieldId, RunTarget};
use crate::tui::{
    input::InputGuard,
    model::{WizardModel, build_policy_label, launch_label, privacy_label, validation_label},
    theme::{Theme, ThemeKind},
};

pub fn summary(model: &WizardModel) -> String {
    model
        .fields
        .iter()
        .filter(|field| !matches!(field.capability.availability, FieldAvailability::Hidden))
        .map(|field| field.capability.label.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn edit_plan(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    plan: DeployPlan,
    theme_kind: ThemeKind,
    no_color: bool,
) -> io::Result<Option<DeployPlan>> {
    let theme = if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
    let mut model = WizardModel::new(plan.normalized());
    model.screen = crate::tui::model::WizardScreen::Options;
    let mut input = InputGuard::default();
    let mut scroll = 0_usize;
    loop {
        terminal.draw(|frame| render(frame, frame.area(), &model, theme, scroll))?;
        if let Some(key) = input.read()? {
            match key {
                KeyCode::Esc => return Ok(None),
                KeyCode::Tab | KeyCode::Down => model.move_focus(1),
                KeyCode::BackTab | KeyCode::Up => model.move_focus(-1),
                KeyCode::PageDown => scroll = scroll.saturating_add(5),
                KeyCode::PageUp => scroll = scroll.saturating_sub(5),
                KeyCode::Left => model.cycle_focused(-1),
                KeyCode::Right => model.cycle_focused(1),
                KeyCode::Char(' ') => model.cycle_focused(1),
                KeyCode::Enter => return Ok(Some(model.plan.normalized())),
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame<'_>, area: Rect, model: &WizardModel, theme: Theme, scroll: usize) {
    let columns = crate::tui::layout::columns(area);
    let lines = options_lines(model, theme);
    let visible = visible_lines(lines, columns[0].height.saturating_sub(2), scroll);
    frame.render_widget(
        Paragraph::new(visible).style(Style::default().bg(theme.panel)).block(
            Block::default()
                .title(" 2. Configure deployment ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent)),
        ),
        columns[0],
    );
    if columns.len() > 1 {
        let context = model.focused_field().map_or_else(
            || "Choose an editable option.".to_owned(),
            |field| {
                format!(
                    "{}\n\n{}\n\n{}",
                    field.capability.label,
                    field.capability.description,
                    availability_reason(&field.capability.availability)
                )
            },
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    model
                        .focused_field()
                        .map_or("Plan", |field| field.capability.label.as_str())
                        .to_owned(),
                    Style::default().fg(theme.selected).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(context, Style::default().fg(theme.text))),
                Line::from(""),
                Line::from(Span::styled(
                    "Left/Right change  |  Tab move  |  Enter preflight",
                    Style::default().fg(theme.success),
                )),
            ])
            .style(Style::default().bg(theme.panel))
            .block(
                Block::default()
                    .title(" 3. Context and next step ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.info)),
            ),
            columns[1],
        );
    }
}

fn options_lines(model: &WizardModel, theme: Theme) -> Vec<Line<'static>> {
    let plan = &model.plan;
    let sections = [
        (
            "CONNECTION",
            vec![row(model, FieldId::ProviderProfile, "Iroh profile", profile_label(plan))],
        ),
        (
            "TARGET & BUILD",
            vec![
                row(model, FieldId::RunWindows, "Run Windows", &checkbox(plan, RunTarget::Windows)),
                row(
                    model,
                    FieldId::RunAndroid,
                    "Run Android device",
                    &checkbox(plan, RunTarget::Android),
                ),
                row(
                    model,
                    FieldId::RunEmulator,
                    "Run Android emulator",
                    &checkbox(plan, RunTarget::Emulator),
                ),
                row(
                    model,
                    FieldId::Configuration,
                    "Configuration",
                    &plan.configuration.to_string(),
                ),
                row(
                    model,
                    FieldId::ClientBuild,
                    "Client build",
                    build_policy_label(plan.client_build),
                ),
            ],
        ),
        (
            "DATA & PRIVACY",
            vec![
                row(model, FieldId::ClientData, "Client data", data_label(plan.client_data)),
                row(model, FieldId::Privacy, "Privacy", privacy_label(plan.privacy)),
            ],
        ),
        (
            "VERIFY & START",
            vec![
                row(model, FieldId::Validation, "Validation", validation_label(plan.validation)),
                row(model, FieldId::Launch, "Launch", launch_label(plan.launch)),
            ],
        ),
    ];
    let mut output = vec![
        Line::from(vec![
            Span::styled("Provider      ", Style::default().fg(theme.muted)),
            Span::styled("Iroh", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Action        ", Style::default().fg(theme.muted)),
            Span::styled(
                plan.action.to_string(),
                Style::default().fg(theme.selected).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];
    for (title, section) in sections {
        let visible: Vec<_> = section.into_iter().filter(|line| !line.is_empty()).collect();
        if !visible.is_empty() {
            output.push(Line::from(Span::styled(
                title,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
            output.extend(visible.into_iter().map(|line| styled_row(line, theme)));
            output.push(Line::from(""));
        }
    }
    let capabilities = model.capabilities();
    output.push(Line::from(vec![
        Span::styled("PLAN  ", Style::default().fg(theme.muted)),
        Span::styled(
            format!(
                "{} steps / about {} min",
                capabilities.estimated_work.steps, capabilities.estimated_work.minutes
            ),
            Style::default().fg(theme.info),
        ),
        Span::styled(
            if capabilities.destructive { "  DESTRUCTIVE" } else { "  SAFE" },
            Style::default()
                .fg(if capabilities.destructive { theme.danger } else { theme.success })
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    output.push(Line::from(""));
    output.push(Line::from(Span::styled(
        "Left/Right change   Tab/Up/Down focus   Enter review   Esc back",
        Style::default().fg(theme.muted),
    )));
    output
}

fn styled_row(row: String, theme: Theme) -> Line<'static> {
    let active = row.starts_with('>');
    let color = if active { theme.selected } else { theme.text };
    Line::from(Span::styled(
        row,
        Style::default().fg(color).add_modifier(if active {
            Modifier::BOLD
        } else {
            Modifier::empty()
        }),
    ))
}

fn visible_lines(
    lines: Vec<Line<'static>>,
    height: u16,
    requested_offset: usize,
) -> Vec<Line<'static>> {
    let capacity = usize::from(height.max(1));
    let max_offset = lines.len().saturating_sub(capacity);
    let offset = requested_offset.min(max_offset);
    lines.into_iter().skip(offset).take(capacity).collect()
}

fn row(model: &WizardModel, id: FieldId, label: &str, value: &str) -> String {
    let Some(field) = model.capability(id) else { return String::new() };
    if matches!(field.availability, FieldAvailability::Hidden) {
        return String::new();
    }
    let marker = if model.focused_field().is_some_and(|focused| focused.capability.id == id) {
        ">"
    } else {
        " "
    };
    let status = match &field.availability {
        FieldAvailability::Editable => String::new(),
        FieldAvailability::ReadOnly { reason } => format!(" - {reason}"),
        FieldAvailability::Disabled { reason } => format!(" - unavailable: {reason}"),
        FieldAvailability::Hidden => String::new(),
    };
    format!("{marker} {label}: {value}{status}")
}

fn availability_reason(availability: &FieldAvailability) -> &str {
    match availability {
        FieldAvailability::Editable => "Editable",
        FieldAvailability::ReadOnly { reason } | FieldAvailability::Disabled { reason } => reason,
        FieldAvailability::Hidden => "Hidden for this plan",
    }
}

fn profile_label(plan: &DeployPlan) -> &str {
    plan.provider_profile.as_deref().unwrap_or("default")
}

fn checkbox(plan: &DeployPlan, target: RunTarget) -> String {
    let selected = plan.clone().normalized().run_targets.contains(&target);
    format!("[{}]", if selected { 'x' } else { ' ' })
}

fn data_label(policy: ClientDataPolicy) -> &'static str {
    match policy {
        ClientDataPolicy::Preserve => "Preserve",
        ClientDataPolicy::ResetProfile => "Reset profile",
        ClientDataPolicy::ResetAll => "Reset all",
    }
}
