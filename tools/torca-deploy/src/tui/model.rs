use crate::domain::{
    BuildPolicy, ClientDataPolicy, Configuration, DeployAction, DeployPlan, DeployRun,
    FieldAvailability, FieldCapability, FieldId, LaunchPolicy, PlanCapabilities, PreflightReport,
    PrivacyPolicy, ProviderMetadataExt, RunTarget, ValidationLevel, iroh_provider,
};
use crate::executor::DeployProgress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WizardScreen {
    Provider,
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

pub fn privacy_label(policy: PrivacyPolicy) -> &'static str {
    match policy {
        PrivacyPolicy::Strict => "Strict (block screenshots/recording)",
        PrivacyPolicy::AllowCapture => "Allow screenshots/recording",
    }
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

pub fn cycle_provider_profile(current: &str, direction: i8) -> Option<String> {
    let profiles = iroh_provider().descriptor().profiles;
    if profiles.is_empty() {
        return None;
    }
    let index = profiles.iter().position(|profile| profile.id == current).unwrap_or(0);
    Some(
        profiles[(index as i8 + direction).rem_euclid(profiles.len() as i8) as usize].id.to_owned(),
    )
}

pub fn marker(active: bool) -> &'static str {
    if active { ">" } else { " " }
}

#[derive(Clone, Debug)]
pub struct WizardModel {
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
        let mut model = Self {
            plan: plan.normalized(),
            fields: Vec::new(),
            focused: 0,
            screen: WizardScreen::Provider,
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
        let previous = self.focused_field().map(|field| field.capability.id);
        self.fields = self
            .plan
            .capabilities()
            .fields
            .into_iter()
            .map(|capability| WizardField { capability })
            .collect();
        self.fields.sort_by_key(|field| field_order(field.capability.id));
        self.focused = previous
            .and_then(|id| self.focusable_index(id))
            .or_else(|| {
                previous.and_then(|id| {
                    let section = field_section(id);
                    self.fields.iter().position(|field| {
                        field_section(field.capability.id) == section
                            && is_focusable(&field.capability.availability)
                    })
                })
            })
            .or_else(|| self.first_focusable())
            .unwrap_or(0);
    }

    pub fn set_action(&mut self, action: DeployAction) {
        self.plan.action = action;
        self.plan = self.plan.clone().normalized();
        self.rebuild_fields();
        self.preflight = None;
    }

    /// Apply one interactive change to the currently focused field.  The
    /// caller never needs to maintain a second copy of the plan values.
    pub fn cycle_focused(&mut self, direction: i8) {
        let Some(field) = self.focused_field().map(|field| field.capability.id) else {
            return;
        };
        if !self.is_editable(field) {
            return;
        }
        match field {
            FieldId::Targets => {}
            FieldId::RunWindows | FieldId::RunAndroid | FieldId::RunEmulator => {
                let target = match field {
                    FieldId::RunWindows => RunTarget::Windows,
                    FieldId::RunAndroid => RunTarget::Android,
                    FieldId::RunEmulator => RunTarget::Emulator,
                    _ => unreachable!(),
                };
                let selected = self.plan.run_targets.contains(&target);
                if selected && self.plan.run_targets.len() == 1 {
                    return;
                }
                if selected {
                    self.plan.run_targets.retain(|current| *current != target);
                } else {
                    self.plan.run_targets.push(target);
                }
                self.plan = self.plan.clone().normalized();
            }
            FieldId::Configuration => {
                self.plan.configuration = if self.plan.configuration == Configuration::Debug {
                    Configuration::Release
                } else {
                    Configuration::Debug
                };
            }
            FieldId::ClientBuild => {
                self.plan.client_build = cycle_build_policy(self.plan.client_build, direction);
            }
            FieldId::ClientData => {
                self.plan.client_data = cycle_data(self.plan.client_data, direction);
            }
            FieldId::Privacy => {
                self.plan.privacy = if self.plan.privacy == PrivacyPolicy::Strict {
                    PrivacyPolicy::AllowCapture
                } else {
                    PrivacyPolicy::Strict
                }
            }
            FieldId::ProviderProfile => {
                self.plan.provider_profile = cycle_provider_profile(
                    self.plan.provider_profile.as_deref().unwrap_or_default(),
                    direction,
                );
            }
            FieldId::Validation => {
                self.plan.validation = cycle_validation(self.plan.validation, direction);
            }
            FieldId::Launch => self.plan.launch = cycle_launch(self.plan.launch, direction),
        }
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

    fn focusable_index(&self, id: FieldId) -> Option<usize> {
        self.fields.iter().position(|field| {
            field.capability.id == id && is_focusable(&field.capability.availability)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FieldSection {
    Connection,
    TargetAndBuild,
    DataAndPrivacy,
    VerifyAndStart,
}

const fn field_section(id: FieldId) -> FieldSection {
    match id {
        FieldId::ProviderProfile => FieldSection::Connection,
        FieldId::Targets
        | FieldId::RunWindows
        | FieldId::RunAndroid
        | FieldId::RunEmulator
        | FieldId::Configuration
        | FieldId::ClientBuild => FieldSection::TargetAndBuild,
        FieldId::ClientData | FieldId::Privacy => FieldSection::DataAndPrivacy,
        FieldId::Validation | FieldId::Launch => FieldSection::VerifyAndStart,
    }
}

const fn field_order(id: FieldId) -> u8 {
    match id {
        FieldId::ProviderProfile => 0,
        FieldId::Targets => 1,
        FieldId::RunWindows => 1,
        FieldId::RunAndroid => 2,
        FieldId::RunEmulator => 3,
        FieldId::Configuration => 4,
        FieldId::ClientBuild => 5,
        FieldId::ClientData => 6,
        FieldId::Privacy => 7,
        FieldId::Validation => 8,
        FieldId::Launch => 9,
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
        assert_eq!(
            model.focused_field().map(|field| field.capability.id),
            Some(FieldId::ProviderProfile)
        );
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
        assert!(model.is_editable(FieldId::Validation));
        assert!(model.is_editable(FieldId::Launch));
        assert_eq!(cycle_validation(ValidationLevel::Full, 1), ValidationLevel::Skip);
        assert_eq!(cycle_launch(LaunchPolicy::Skip, -1), LaunchPolicy::Restart);
    }

    #[test]
    fn iroh_profile_capability_is_present() {
        let model = WizardModel::new(DeployPlan::normal(
            DeployAction::FullRedeploy,
            vec![Target::Windows, Target::Android],
            Configuration::Debug,
        ));
        assert!(model.capability(FieldId::ProviderProfile).is_some());
    }

    #[test]
    fn cycling_focused_target_updates_runtime_checkbox_sets() {
        let mut model = WizardModel::new(DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Windows],
            Configuration::Debug,
        ));
        model.move_focus(1);
        model.move_focus(1);
        assert_eq!(
            model.focused_field().map(|field| field.capability.id),
            Some(FieldId::RunAndroid)
        );
        model.cycle_focused(1);
        assert_eq!(model.plan.run_targets, vec![RunTarget::Windows, RunTarget::Android]);
        assert!(model.plan.targets.contains(&Target::Android));
        assert_eq!(
            model.focused_field().map(|field| field.capability.id),
            Some(FieldId::RunAndroid)
        );
        model.cycle_focused(1);
        assert_eq!(model.plan.run_targets, vec![RunTarget::Windows]);
        assert_eq!(
            model.focused_field().map(|field| field.capability.id),
            Some(FieldId::RunAndroid)
        );
    }

    #[test]
    fn dynamic_rebuild_keeps_focus_on_the_same_visible_field() {
        let mut model = WizardModel::new(DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Android],
            Configuration::Debug,
        ));
        model.focused = model
            .fields
            .iter()
            .position(|field| field.capability.id == FieldId::Privacy)
            .expect("privacy field");

        model.cycle_focused(1);

        assert_eq!(model.focused_field().map(|field| field.capability.id), Some(FieldId::Privacy));
        assert_eq!(model.plan.privacy, PrivacyPolicy::AllowCapture);
    }

    #[test]
    fn hidden_field_falls_back_to_first_editable_field_in_its_section() {
        let mut model = WizardModel::new(DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Windows],
            Configuration::Debug,
        ));
        model.focused = model
            .fields
            .iter()
            .position(|field| field.capability.id == FieldId::Configuration)
            .expect("configuration field");

        model.set_action(DeployAction::RunInstalled);

        assert_eq!(
            model.focused_field().map(|field| field.capability.id),
            Some(FieldId::RunWindows)
        );
    }

    #[test]
    fn focus_navigation_order_matches_the_rendered_sections() {
        let model = WizardModel::new(DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Windows],
            Configuration::Debug,
        ));
        let ids = model.fields.iter().map(|field| field.capability.id).collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                FieldId::ProviderProfile,
                FieldId::RunWindows,
                FieldId::RunAndroid,
                FieldId::RunEmulator,
                FieldId::Configuration,
                FieldId::ClientBuild,
                FieldId::ClientData,
                FieldId::Privacy,
                FieldId::Validation,
                FieldId::Launch,
            ]
        );
    }
}
