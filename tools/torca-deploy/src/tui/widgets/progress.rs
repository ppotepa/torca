use crate::tui::model::ExecutionDashboard;

pub fn label(dashboard: &ExecutionDashboard) -> String {
    format!("{}/{}", dashboard.completed_steps, dashboard.total_steps)
}
