use crate::domain::{Configuration, DeployAction, DeployPlan};

pub fn details(action: DeployAction) -> String {
    let plan =
        DeployPlan::normal(action, crate::planner::all_client_targets(), Configuration::Debug);
    format!(
        "{}\n\nWill execute:\n{}\n\nEstimated work: {} steps / about {} min\nDestructive: {}",
        action,
        crate::tui::widgets::plan_preview::steps_text(&plan),
        plan.capabilities().estimated_work.steps,
        plan.capabilities().estimated_work.minutes,
        if plan.capabilities().destructive { "yes" } else { "no" },
    )
}
