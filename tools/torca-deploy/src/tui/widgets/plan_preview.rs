use crate::domain::{DeployPlan, StepDisposition};

pub fn step_marker(disposition: StepDisposition) -> &'static str {
    match disposition {
        StepDisposition::Execute => "✓",
        StepDisposition::Reuse => "→",
        StepDisposition::Skip => "○",
        StepDisposition::Blocked => "✗",
    }
}

pub fn steps_text(plan: &DeployPlan) -> String {
    plan.planned_steps()
        .into_iter()
        .map(|step| format!("{} {}", step_marker(step.disposition), step.label))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Configuration, DeployAction, Target};

    #[test]
    fn preview_uses_the_same_steps_as_the_plan() {
        let plan = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        assert!(steps_text(&plan).contains("Build artifacts"));
        assert!(steps_text(&plan).contains("→"));
    }
}
