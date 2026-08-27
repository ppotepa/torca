use crate::domain::{
    BuildPolicy, ClientDataPolicy, CommunicationProvider, Configuration, DeployAction, DeployPlan,
    DeployRun, FieldAvailability, FieldCapability, FieldId, LaunchPolicy, PlanCapabilities,
    PreflightReport, PrivacyPolicy, ProviderMaintenancePolicy, Target, ValidationLevel,
};
use crate::executor::DeployProgress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WizardScreen {
    Action,
    Options,
    Devices,
    Preflight,
    Review,
    Execution,
    Failure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WizardField {
    pub capability: FieldCapability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    Target,
    Configuration,
    ClientBuild,
    ProviderServiceBuild,
    ClientData,
    ProviderMaintenance,
    Privacy,
    CommunicationProvider,
    ProviderProfile,
    Validation,
    Launch,
}

pub fn draft_plan(
    action: DeployAction,
    target: u8,
    configuration: Configuration,
    client_build: BuildPolicy,
    provider_service_build: BuildPolicy,
    client_data: ClientDataPolicy,
    provider_maintenance: ProviderMaintenancePolicy,
    privacy: PrivacyPolicy,
    communication_provider: CommunicationProvider,
    provider_profile: Option<String>,
    validation: ValidationLevel,
    launch: LaunchPolicy,
) -> DeployPlan {
    let targets = match target {
        1 => vec![Target::Windows],
        2 => vec![Target::Android],
        _ => crate::planner::all_client_targets(),
    };
    let mut plan = DeployPlan::normal(action, targets, configuration);
    plan.client_build = client_build;
    plan.provider_service_build = provider_service_build;
    plan.client_data = client_data;
    plan.provider_maintenance = provider_maintenance;
    plan.privacy = privacy;
    plan.communication_provider = communication_provider;
    plan.provider_profile = provider_profile;
    plan.validation = validation;
    plan.launch = launch;
    plan
}

pub fn field_id(field: Field) -> FieldId {
    match field {
        Field::Target => FieldId::Targets,
        Field::Configuration => FieldId::Configuration,
        Field::ClientBuild => FieldId::ClientBuild,
        Field::ProviderServiceBuild => FieldId::ProviderServiceBuild,
        Field::ClientData => FieldId::ClientData,
        Field::ProviderMaintenance => FieldId::ProviderMaintenance,
        Field::Privacy => FieldId::Privacy,
        Field::CommunicationProvider => FieldId::CommunicationProvider,
        Field::ProviderProfile => FieldId::ProviderProfile,
        Field::Validation => FieldId::Validation,
        Field::Launch => FieldId::Launch,
    }
}

pub fn privacy_label(policy: PrivacyPolicy) -> &'static str {
    match policy {
        PrivacyPolicy::Strict => "Strict (block screenshots/recording)",
        PrivacyPolicy::AllowCapture => "Allow screenshots/recording",
    }
}

pub fn field_is_editable(plan: &DeployPlan, field: Field) -> bool {
    WizardModel::new(plan.clone()).is_editable(field_id(field))
}

pub fn next_field_for_plan(current: Field, plan: &DeployPlan, direction: i8) -> Field {
    let mut field = current;
    for _ in 0..10 {
        field = if direction >= 0 { next_field(field) } else { previous_field(field) };
        if field_is_editable(plan, field) {
            return field;
        }
    }
    current
}

pub fn provider_profile_description(
    provider: CommunicationProvider,
    profile: &str,
) -> &'static str {
    provider
        .descriptor()
        .profiles
        .iter()
        .find(|candidate| candidate.id == profile)
        .map_or("provider default", |candidate| candidate.description)
}

pub fn build_policy_label(policy: BuildPolicy) -> &'static str {
    match policy {
        BuildPolicy::Reuse => "Reuse",
        BuildPolicy::IfRequired => "If required",
        BuildPolicy::Rebuild => "Rebuild",
    }
}

pub fn cycle_build_policy(current: BuildPolicy, direction: i8) -> BuildPolicy {
    let index = match current {
        BuildPolicy::Reuse => 0,
        BuildPolicy::IfRequired => 1,
        BuildPolicy::Rebuild => 2,
    };
    match (index as i8 + direction).rem_euclid(3) {
        0 => BuildPolicy::Reuse,
        2 => BuildPolicy::Rebuild,
        _ => BuildPolicy::IfRequired,
    }
}

pub fn validation_label(level: ValidationLevel) -> &'static str {
    match level {
        ValidationLevel::Skip => "Skip",
        ValidationLevel::Quick => "Quick",
        ValidationLevel::Full => "Full",
    }
}

pub fn cycle_validation(level: ValidationLevel, direction: i8) -> ValidationLevel {
    let index = match level {
        ValidationLevel::Skip => 0,
        ValidationLevel::Quick => 1,
        ValidationLevel::Full => 2,
    };
    match (index as i8 + direction).rem_euclid(3) {
        0 => ValidationLevel::Skip,
        2 => ValidationLevel::Full,
        _ => ValidationLevel::Quick,
    }
}

pub fn launch_label(policy: LaunchPolicy) -> &'static str {
    match policy {
        LaunchPolicy::Skip => "Skip",
        LaunchPolicy::Start => "Start",
        LaunchPolicy::Restart => "Restart",
    }
}

pub fn cycle_launch(policy: LaunchPolicy, direction: i8) -> LaunchPolicy {
    let index = match policy {
        LaunchPolicy::Skip => 0,
        LaunchPolicy::Start => 1,
        LaunchPolicy::Restart => 2,
    };
    match (index as i8 + direction).rem_euclid(3) {
        0 => LaunchPolicy::Skip,
        2 => LaunchPolicy::Restart,
        _ => LaunchPolicy::Start,
    }
}

fn next_field(field: Field) -> Field {
    match field {
        Field::Target => Field::Configuration,
        Field::Configuration => Field::ClientBuild,
        Field::ClientBuild => Field::ProviderServiceBuild,
        Field::ProviderServiceBuild => Field::ClientData,
        Field::ClientData => Field::ProviderMaintenance,
        Field::ProviderMaintenance => Field::Privacy,
        Field::Privacy => Field::CommunicationProvider,
        Field::CommunicationProvider => Field::ProviderProfile,
        Field::ProviderProfile => Field::Validation,
        Field::Validation => Field::Launch,
        Field::Launch => Field::Target,
    }
}

fn previous_field(field: Field) -> Field {
    match field {
        Field::Target => Field::Launch,
        Field::Configuration => Field::Target,
        Field::ClientBuild => Field::Configuration,
        Field::ProviderServiceBuild => Field::ClientBuild,
        Field::ClientData => Field::ProviderServiceBuild,
        Field::ProviderMaintenance => Field::ClientData,
        Field::Privacy => Field::ProviderMaintenance,
        Field::CommunicationProvider => Field::Privacy,
        Field::ProviderProfile => Field::CommunicationProvider,
        Field::Validation => Field::ProviderProfile,
        Field::Launch => Field::Validation,
    }
}

pub fn cycle_data(current: ClientDataPolicy, direction: i8) -> ClientDataPolicy {
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

pub fn cycle_target(current: u8, direction: i8) -> u8 {
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

pub fn cycle_provider(current: CommunicationProvider, direction: i8) -> CommunicationProvider {
    let providers = CommunicationProvider::selectable();
    let index = providers.iter().position(|provider| *provider == current).unwrap_or(0);
    let next = (index as i8 + direction).rem_euclid(providers.len() as i8) as usize;
    providers[next]
}

pub fn cycle_provider_profile(
    provider: CommunicationProvider,
    current: &str,
    direction: i8,
) -> Option<String> {
    let profiles = provider.descriptor().profiles;
    if profiles.is_empty() {
        return None;
    }
    let index = profiles.iter().position(|profile| profile.id == current).unwrap_or(0);
    Some(
        profiles[(index as i8 + direction).rem_euclid(profiles.len() as i8) as usize].id.to_owned(),
    )
}

pub fn cycle_provider_maintenance(
    provider: CommunicationProvider,
    current: ProviderMaintenancePolicy,
    direction: i8,
) -> ProviderMaintenancePolicy {
    let options = provider.descriptor().maintenance;
    if options.is_empty() {
        return ProviderMaintenancePolicy::Ensure;
    }
    let current = match current {
        ProviderMaintenancePolicy::Ensure => torca_transport_api::MaintenanceOption::Ensure,
        ProviderMaintenancePolicy::Restart => torca_transport_api::MaintenanceOption::Restart,
        ProviderMaintenancePolicy::RepairDirectoryCache => {
            torca_transport_api::MaintenanceOption::RepairDirectoryCache
        }
        ProviderMaintenancePolicy::RotateIdentity => {
            torca_transport_api::MaintenanceOption::RotateIdentity
        }
    };
    let index = options.iter().position(|option| *option == current).unwrap_or(0);
    match options[(index as i8 + direction).rem_euclid(options.len() as i8) as usize] {
        torca_transport_api::MaintenanceOption::Ensure => ProviderMaintenancePolicy::Ensure,
        torca_transport_api::MaintenanceOption::Restart => ProviderMaintenancePolicy::Restart,
        torca_transport_api::MaintenanceOption::RepairDirectoryCache => {
            ProviderMaintenancePolicy::RepairDirectoryCache
        }
        torca_transport_api::MaintenanceOption::RotateIdentity => {
            ProviderMaintenancePolicy::RotateIdentity
        }
    }
}

pub fn marker(active: bool) -> &'static str {
    if active { ">" } else { " " }
}

#[derive(Clone, Debug)]
pub struct WizardModel {
    pub action: DeployAction,
    pub plan: DeployPlan,
    pub fields: Vec<WizardField>,
    pub focused: usize,
    pub screen: WizardScreen,
    pub preflight: Option<PreflightReport>,
}

#[derive(Clone, Debug)]
pub struct ExecutionDashboard {
    pub run: DeployRun,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub current_operation: Option<String>,
    pub recent_output: Vec<String>,
    pub raw_logs: bool,
    pub paused: bool,
    pub cancel_requested: bool,
    pub diagnostics_status: Option<String>,
    scroll: usize,
    pending_progress: Option<DeployProgress>,
}

impl ExecutionDashboard {
    pub fn new(run: DeployRun) -> Self {
        let total_steps = run.plan.planned_steps().len();
        let completed_steps = run.completed.len().min(total_steps);
        Self {
            run,
            total_steps,
            completed_steps,
            current_operation: None,
            recent_output: Vec::new(),
            raw_logs: false,
            paused: false,
            cancel_requested: false,
            diagnostics_status: None,
            scroll: 0,
            pending_progress: None,
        }
    }

    pub fn toggle_raw_logs(&mut self) {
        self.raw_logs = !self.raw_logs;
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if !self.paused {
            if let Some(progress) = self.pending_progress.take() {
                self.apply_progress(progress);
            }
        }
    }

    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
    }

    pub fn clear_cancel_request(&mut self) {
        self.cancel_requested = false;
    }

    pub fn receive_progress(&mut self, progress: DeployProgress) {
        if self.paused {
            self.pending_progress = Some(progress);
        } else {
            self.apply_progress(progress);
        }
    }

    pub fn set_diagnostics_status(&mut self, status: impl Into<String>) {
        self.diagnostics_status = Some(status.into());
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(5);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(5);
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll
    }

    pub fn apply_progress(&mut self, progress: DeployProgress) {
        self.run.stage = progress.stage;
        self.run.message = Some(progress.message.clone());
        self.completed_steps = progress.completed_steps;
        self.total_steps = progress.total_steps;
        self.current_operation = Some(progress.message.clone());
        self.push_output(progress.message);
    }

    pub fn push_output(&mut self, line: impl Into<String>) {
        const MAX_LINES: usize = 8;
        self.recent_output.push(line.into());
        if self.recent_output.len() > MAX_LINES {
            let excess = self.recent_output.len() - MAX_LINES;
            self.recent_output.drain(..excess);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureAction {
    RetryFailedStage,
    EditPlan,
    Diagnostics,
    CollectLogs,
    Quit,
}

impl WizardModel {
    pub fn new(plan: DeployPlan) -> Self {
        let action = plan.action;
        let mut model = Self {
            action,
            plan,
            fields: Vec::new(),
            focused: 0,
            screen: WizardScreen::Action,
            preflight: None,
        };
        model.rebuild_fields();
        model
    }

    pub fn capabilities(&self) -> PlanCapabilities {
        self.plan.capabilities()
    }

    pub fn planned_steps(&self) -> Vec<crate::domain::PlannedStep> {
        self.plan.planned_steps()
    }

    pub fn capability(&self, id: FieldId) -> Option<&FieldCapability> {
        self.fields.iter().find(|field| field.capability.id == id).map(|field| &field.capability)
    }

    pub fn is_editable(&self, id: FieldId) -> bool {
        self.capability(id)
            .is_some_and(|field| matches!(field.availability, FieldAvailability::Editable))
    }

    pub fn rebuild_fields(&mut self) {
        self.fields = self
            .plan
            .capabilities()
            .fields
            .into_iter()
            .map(|capability| WizardField { capability })
            .collect();
        self.focused = self.first_focusable().unwrap_or(0);
    }

    pub fn set_action(&mut self, action: DeployAction) {
        self.action = action;
        self.plan.action = action;
        self.plan = self.plan.clone().normalized();
        self.rebuild_fields();
        self.preflight = None;
    }

    pub fn set_provider(&mut self, provider: crate::domain::CommunicationProvider) {
        self.plan.communication_provider = provider;
        self.plan = self.plan.clone().normalized();
        self.rebuild_fields();
        self.preflight = None;
    }

    pub fn set_preflight(&mut self, report: PreflightReport) {
        self.preflight = Some(report);
        self.screen = WizardScreen::Preflight;
    }

    pub fn can_execute(&self) -> bool {
        self.preflight.as_ref().is_some_and(|report| report.can_execute)
    }

    pub fn focused_field(&self) -> Option<&WizardField> {
        self.fields.get(self.focused)
    }

    pub fn move_focus(&mut self, delta: i32) {
        if self.fields.is_empty() {
            return;
        }
        let len = self.fields.len();
        for _ in 0..self.fields.len() {
            self.focused =
                if delta >= 0 { (self.focused + 1) % len } else { (self.focused + len - 1) % len };
            if is_focusable(&self.fields[self.focused].capability.availability) {
                break;
            }
        }
    }

    fn first_focusable(&self) -> Option<usize> {
        self.fields.iter().position(|field| is_focusable(&field.capability.availability))
    }
}

fn is_focusable(availability: &FieldAvailability) -> bool {
    matches!(availability, FieldAvailability::Editable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Configuration, FieldId, Target};

    #[test]
    fn focus_skips_hidden_and_read_only_fields() {
        let model = WizardModel::new(DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        ));
        assert_eq!(model.focused_field().map(|field| field.capability.id), Some(FieldId::Targets));
        assert!(
            model
                .fields
                .iter()
                .filter(|field| matches!(
                    field.capability.availability,
                    FieldAvailability::ReadOnly { .. } | FieldAvailability::Hidden
                ))
                .all(|field| Some(field) != model.focused_field())
        );
    }

    #[test]
    fn preflight_blocker_disables_execution_until_rechecked() {
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        let mut model = WizardModel::new(plan);
        model.set_preflight(PreflightReport { checks: Vec::new(), can_execute: false });
        assert!(!model.can_execute());
        model.set_action(DeployAction::Rebuild);
        assert!(model.preflight.is_none());
    }

    #[test]
    fn execution_dashboard_bounds_recent_output() {
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        let mut dashboard = ExecutionDashboard::new(DeployRun::new(plan));
        for index in 0..20 {
            dashboard.push_output(format!("line {index}"));
        }
        assert_eq!(dashboard.recent_output.len(), 8);
        assert_eq!(dashboard.recent_output.first().map(String::as_str), Some("line 12"));
    }

    #[test]
    fn paused_dashboard_keeps_latest_progress_until_resumed() {
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        let mut dashboard = ExecutionDashboard::new(DeployRun::new(plan));
        dashboard.toggle_pause();
        dashboard.receive_progress(DeployProgress {
            run_id: "run".into(),
            stage: crate::domain::DeployStage::ArtifactsBuilt,
            completed_steps: 2,
            total_steps: 4,
            message: "building".into(),
        });
        assert_eq!(dashboard.completed_steps, 0);
        dashboard.toggle_pause();
        assert_eq!(dashboard.completed_steps, 2);
        assert_eq!(dashboard.recent_output, vec!["building"]);
    }

    #[test]
    fn execution_dashboard_scroll_is_bounded_at_zero() {
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        let mut dashboard = ExecutionDashboard::new(DeployRun::new(plan));
        dashboard.scroll_up();
        assert_eq!(dashboard.scroll_offset(), 0);
        dashboard.scroll_down();
        assert_eq!(dashboard.scroll_offset(), 5);
    }

    #[test]
    fn cancellation_requires_an_explicit_confirmation() {
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        let mut dashboard = ExecutionDashboard::new(DeployRun::new(plan));
        assert!(!dashboard.cancel_requested);
        dashboard.request_cancel();
        assert!(dashboard.cancel_requested);
        dashboard.clear_cancel_request();
        assert!(!dashboard.cancel_requested);
    }

    #[test]
    fn build_validation_and_launch_fields_follow_action_capabilities() {
        let model = WizardModel::new(DeployPlan::normal(
            DeployAction::Rebuild,
            vec![Target::Android],
            Configuration::Debug,
        ));
        assert!(!model.is_editable(FieldId::ClientBuild));
        assert!(!model.is_editable(FieldId::ProviderServiceBuild));
        assert!(model.is_editable(FieldId::Validation));
        assert!(model.is_editable(FieldId::Launch));
        assert_eq!(cycle_validation(ValidationLevel::Full, 1), ValidationLevel::Skip);
        assert_eq!(cycle_launch(LaunchPolicy::Skip, -1), LaunchPolicy::Restart);
    }
}
