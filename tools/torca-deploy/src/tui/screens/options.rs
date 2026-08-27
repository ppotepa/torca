use crate::tui::{
    model::WizardModel,
    widgets::{capability_badge, field_editor},
};

use std::io;

use crossterm::event::KeyCode;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

use crate::domain::{
    BuildPolicy, ClientDataPolicy, CommunicationProvider, Configuration, DeployAction, DeployPlan,
    LaunchPolicy, PrivacyPolicy, ProviderMaintenancePolicy, Target, ValidationLevel,
};
use crate::tui::{
    input::InputGuard,
    model::{
        Field, WizardModel as PlanWizardModel, build_policy_label, cycle_build_policy, cycle_data,
        cycle_launch, cycle_provider, cycle_provider_maintenance, cycle_provider_profile,
        cycle_target, cycle_validation, draft_plan, field_is_editable, launch_label, marker,
        next_field_for_plan, privacy_label, provider_profile_description, validation_label,
    },
    theme::{Theme, ThemeKind},
};

pub fn summary(model: &WizardModel) -> String {
    model
        .fields
        .iter()
        .filter(|field| field.capability.availability != crate::domain::FieldAvailability::Hidden)
        .map(|field| {
            format!(
                "{} ({})",
                field_editor::describe(&field.capability),
                capability_badge::availability_label(&field.capability.availability)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Owns the contextual options screen. The parent TUI only chooses when to
/// enter this screen and receives the resulting normalized plan.
pub fn edit_plan(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    action: DeployAction,
    theme_kind: ThemeKind,
    no_color: bool,
) -> io::Result<Option<DeployPlan>> {
    let theme = if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
    let mut field = Field::Target;
    let mut target = 0_u8;
    let mut configuration = Configuration::Debug;
    let mut client_build = BuildPolicy::IfRequired;
    let mut provider_service_build = BuildPolicy::IfRequired;
    let mut client_data = if action == DeployAction::FullRedeploy {
        ClientDataPolicy::ResetProfile
    } else {
        ClientDataPolicy::Preserve
    };
    let mut provider_maintenance = ProviderMaintenancePolicy::Ensure;
    let mut privacy = PrivacyPolicy::Strict;
    let mut communication_provider = CommunicationProvider::Tor;
    let mut provider_profile: Option<String> = None;
    let mut validation = ValidationLevel::Quick;
    let mut launch = LaunchPolicy::Restart;
    let mut input = InputGuard::default();
    let mut scroll = 0_usize;

    loop {
        let draft = draft_plan(
            action,
            target,
            configuration,
            client_build,
            provider_service_build,
            client_data,
            provider_maintenance,
            privacy,
            communication_provider,
            provider_profile.clone(),
            validation,
            launch,
        );
        let wizard_model = PlanWizardModel::new(draft.clone());
        terminal.draw(|frame| {
            render(
                frame,
                frame.area(),
                &wizard_model,
                &draft,
                action,
                field,
                target,
                communication_provider,
                provider_profile.as_deref(),
                theme,
                scroll,
            );
        })?;

        if let Some(key) = input.read()? {
            match key {
                KeyCode::Esc => return Ok(None),
                KeyCode::Tab | KeyCode::Down => {
                    field = next_field_for_plan(field, &draft, 1);
                }
                KeyCode::BackTab | KeyCode::Up => field = next_field_for_plan(field, &draft, -1),
                KeyCode::PageDown => scroll = scroll.saturating_add(5),
                KeyCode::PageUp => scroll = scroll.saturating_sub(5),
                KeyCode::Left | KeyCode::Right => {
                    let direction = if key == KeyCode::Right { 1_i8 } else { -1 };
                    if !field_is_editable(&draft, field) {
                        continue;
                    }
                    match field {
                        Field::Target => target = cycle_target(target, direction),
                        Field::Configuration => {
                            configuration = if configuration == Configuration::Debug {
                                Configuration::Release
                            } else {
                                Configuration::Debug
                            };
                        }
                        Field::ClientBuild => {
                            client_build = cycle_build_policy(client_build, direction);
                        }
                        Field::ProviderServiceBuild => {
                            provider_service_build =
                                cycle_build_policy(provider_service_build, direction);
                        }
                        Field::ClientData => {
                            client_data = cycle_data(client_data, direction);
                        }
                        Field::ProviderMaintenance => {
                            provider_maintenance = cycle_provider_maintenance(
                                communication_provider,
                                provider_maintenance,
                                direction,
                            );
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
                            if communication_provider.descriptor().profiles.is_empty() {
                                provider_profile = None;
                            } else if provider_profile.is_none() {
                                provider_profile = Some(
                                    communication_provider.descriptor().profiles[0].id.to_owned(),
                                );
                            }
                        }
                        Field::ProviderProfile => {
                            provider_profile = cycle_provider_profile(
                                communication_provider,
                                provider_profile.as_deref().unwrap_or_default(),
                                direction,
                            );
                        }
                        Field::Validation => {
                            validation = cycle_validation(validation, direction);
                        }
                        Field::Launch => {
                            launch = cycle_launch(launch, direction);
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
                    plan.client_build = client_build;
                    plan.provider_service_build = provider_service_build;
                    plan.client_data = client_data;
                    plan.provider_maintenance = provider_maintenance;
                    plan.privacy = privacy;
                    plan.communication_provider = communication_provider;
                    plan.provider_profile.clone_from(&provider_profile);
                    plan.validation = validation;
                    plan.launch = launch;
                    return Ok(Some(plan.normalized()));
                }
                _ => {}
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    wizard_model: &PlanWizardModel,
    draft: &DeployPlan,
    action: DeployAction,
    field: Field,
    target: u8,
    communication_provider: CommunicationProvider,
    provider_profile: Option<&str>,
    theme: Theme,
    scroll: usize,
) {
    let displayed = draft.clone().normalized();
    let target_label = match target {
        1 => "Windows",
        2 => "Android",
        _ => "All detected clients",
    };
    let data_label = match displayed.client_data {
        ClientDataPolicy::Preserve => "Preserve client data",
        ClientDataPolicy::ResetProfile => "Reset profile",
        ClientDataPolicy::ResetAll => "Reset all client data",
    };
    let provider_maintenance_label = match displayed.provider_maintenance {
        ProviderMaintenancePolicy::Ensure => "Ensure provider service",
        ProviderMaintenancePolicy::Restart => "Restart provider service, preserve identity",
        ProviderMaintenancePolicy::RepairDirectoryCache => "Repair provider local state",
        ProviderMaintenancePolicy::RotateIdentity => "Rotate provider identity (rebuild all)",
    };
    let provider_profile_label = provider_profile.unwrap_or("default");
    let provider_profile_help =
        provider_profile_description(communication_provider, provider_profile_label);
    let capabilities = wizard_model.capabilities();
    let field_row = |id: crate::domain::FieldId, marker_text: &str, label: &str, value: &str| {
        let Some(capability) = capabilities.fields.iter().find(|item| item.id == id) else {
            return String::new();
        };
        match &capability.availability {
            crate::domain::FieldAvailability::Hidden => String::new(),
            crate::domain::FieldAvailability::Editable => format!("{marker_text} {label}: {value}"),
            crate::domain::FieldAvailability::ReadOnly { reason }
            | crate::domain::FieldAvailability::Disabled { reason } => {
                format!("{marker_text} {label}: {value} ({reason})")
            }
        }
    };
    let text = format!(
        "Action: {action}\n\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n  {provider_profile_help}\n\nContextual fields: {}\n\n←/→ change   Tab/↑/↓ field   Enter review   Esc back",
        field_row(
            crate::domain::FieldId::Targets,
            marker(matches!(field, Field::Target)),
            "Target",
            target_label
        ),
        field_row(
            crate::domain::FieldId::Configuration,
            marker(matches!(field, Field::Configuration)),
            "Build configuration",
            &displayed.configuration.to_string()
        ),
        field_row(
            crate::domain::FieldId::ClientBuild,
            " ",
            "Client build",
            build_policy_label(displayed.client_build)
        ),
        field_row(
            crate::domain::FieldId::ProviderServiceBuild,
            " ",
            "Provider service build",
            build_policy_label(displayed.provider_service_build)
        ),
        field_row(
            crate::domain::FieldId::ClientData,
            marker(matches!(field, Field::ClientData)),
            "Client data",
            data_label
        ),
        field_row(
            crate::domain::FieldId::ProviderMaintenance,
            marker(matches!(field, Field::ProviderMaintenance)),
            "Provider maintenance",
            provider_maintenance_label
        ),
        field_row(
            crate::domain::FieldId::Privacy,
            marker(matches!(field, Field::Privacy)),
            "Privacy",
            privacy_label(displayed.privacy)
        ),
        field_row(
            crate::domain::FieldId::CommunicationProvider,
            marker(matches!(field, Field::CommunicationProvider)),
            "Communication protocol",
            communication_provider.protocol_label()
        ),
        field_row(
            crate::domain::FieldId::ProviderProfile,
            marker(matches!(field, Field::ProviderProfile)),
            "Provider profile",
            provider_profile_label
        ),
        field_row(
            crate::domain::FieldId::Validation,
            marker(matches!(field, Field::Validation)),
            "Validation",
            validation_label(displayed.validation),
        ),
        field_row(
            crate::domain::FieldId::Launch,
            marker(matches!(field, Field::Launch)),
            "Launch",
            launch_label(displayed.launch),
        ),
        summary(wizard_model).replace('\n', ", "),
    );
    frame.render_widget(
        Paragraph::new(crate::tui::layout::viewport(&text, area.height.saturating_sub(2), scroll))
            .style(Style::default().fg(theme.text).bg(theme.panel))
            .block(Block::default().title("Deployment options").borders(Borders::ALL)),
        area,
    );
}
