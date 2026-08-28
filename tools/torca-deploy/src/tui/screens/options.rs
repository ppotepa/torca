use std::{fmt::Write, io};

use crossterm::event::KeyCode;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

use crate::domain::{
    ClientDataPolicy, CommunicationProvider, Configuration, DeployAction, DeployPlan,
    FieldAvailability, FieldId, ProviderMaintenancePolicy, ProviderMetadataExt, Target,
    iroh_provider,
};
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
    action: DeployAction,
    theme_kind: ThemeKind,
    no_color: bool,
) -> io::Result<Option<DeployPlan>> {
    edit_plan_for_provider(terminal, action, iroh_provider(), theme_kind, no_color)
}

pub fn edit_plan_for_provider(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    action: DeployAction,
    provider: CommunicationProvider,
    theme_kind: ThemeKind,
    no_color: bool,
) -> io::Result<Option<DeployPlan>> {
    let theme = if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
    let mut plan =
        DeployPlan::normal(action, crate::planner::all_client_targets(), Configuration::Debug);
    plan.communication_provider = provider;
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
                KeyCode::Enter => return Ok(Some(model.plan.normalized())),
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame<'_>, area: Rect, model: &WizardModel, theme: Theme, scroll: usize) {
    let columns = crate::tui::layout::columns(area);
    let text = options_text(model);
    frame.render_widget(
        Paragraph::new(crate::tui::layout::viewport(
            &text,
            columns[0].height.saturating_sub(2),
            scroll,
        ))
        .style(Style::default().fg(theme.text).bg(theme.panel))
        .block(
            Block::default()
                .title("Options Â· Provider â†’ Action â†’ Options")
                .borders(Borders::ALL),
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
            Paragraph::new(context)
                .style(Style::default().fg(theme.info).bg(theme.panel))
                .block(Block::default().title("Context").borders(Borders::ALL)),
            columns[1],
        );
    }
}

fn options_text(model: &WizardModel) -> String {
    let plan = &model.plan;
    let provider = plan.communication_provider.descriptor();
    let sections = [
        (
            "CONNECTION",
            vec![
                row(model, FieldId::ProviderProfile, "Provider profile", profile_label(plan)),
                format!(
                    "  Provider service: {}",
                    if provider.managed_service { "managed" } else { "not required" }
                ),
            ],
        ),
        (
            "TARGET & BUILD",
            vec![
                row(model, FieldId::Targets, "Targets", targets_label(&plan.targets)),
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
                row(
                    model,
                    FieldId::ProviderServiceBuild,
                    "Provider build",
                    build_policy_label(plan.provider_service_build),
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
        (
            "MAINTENANCE",
            vec![row(
                model,
                FieldId::ProviderMaintenance,
                "Provider maintenance",
                maintenance_label(plan.provider_maintenance),
            )],
        ),
    ];
    let mut output = format!(
        "Provider: {}\n{}\n\nAction: {}\n\n",
        provider.label, provider.description, plan.action
    );
    for (title, section) in sections {
        let visible: Vec<_> = section.into_iter().filter(|line| !line.is_empty()).collect();
        if !visible.is_empty() {
            output.push_str(title);
            output.push('\n');
            output.push_str(&visible.join("\n"));
            output.push_str("\n\n");
        }
    }
    let capabilities = model.capabilities();
    write!(
        output,
        "Plan: {} steps Â· {} min Â· {}\n\nâ†/â†’ change   Tab/â†‘/â†“ focus   Enter review   Esc back",
        capabilities.estimated_work.steps,
        capabilities.estimated_work.minutes,
        if capabilities.destructive { "DESTRUCTIVE" } else { "non-destructive" }
    )
    .expect("writing to a String cannot fail");
    output
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
        FieldAvailability::ReadOnly { reason } => format!(" â€” {reason}"),
        FieldAvailability::Disabled { reason } => format!(" â€” unavailable: {reason}"),
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

fn targets_label(targets: &[Target]) -> &'static str {
    match targets {
        [Target::Windows] => "Windows",
        [Target::Android] => "Android",
        _ => "Windows + Android",
    }
}

fn data_label(policy: ClientDataPolicy) -> &'static str {
    match policy {
        ClientDataPolicy::Preserve => "Preserve",
        ClientDataPolicy::ResetProfile => "Reset profile",
        ClientDataPolicy::ResetAll => "Reset all",
    }
}

fn maintenance_label(policy: ProviderMaintenancePolicy) -> &'static str {
    match policy {
        ProviderMaintenancePolicy::Ensure => "Ensure",
        ProviderMaintenancePolicy::Restart => "Restart",
        ProviderMaintenancePolicy::RepairDirectoryCache => "Repair directory cache",
        ProviderMaintenancePolicy::RotateIdentity => "Rotate identity",
    }
}
