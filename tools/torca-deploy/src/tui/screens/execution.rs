use crate::tui::model::ExecutionDashboard;
use crate::tui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

pub fn title(dashboard: &ExecutionDashboard) -> String {
    format!(
        "TORCA DEPLOY  RUNNING\nOverall progress: {}",
        crate::tui::widgets::progress::label(dashboard)
    )
}

pub fn render(frame: &mut Frame<'_>, area: Rect, dashboard: &ExecutionDashboard, theme: Theme) {
    let output = if dashboard.recent_output.is_empty() {
        "(no output yet)".to_owned()
    } else {
        crate::tui::widgets::log_tail::tail(&dashboard.recent_output, 8).join("\n")
    };
    let view_mode = if dashboard.raw_logs { "raw logs" } else { "summary" };
    let pause_state = if dashboard.paused { "paused" } else { "live" };
    let diagnostics = dashboard.diagnostics_status.as_deref().unwrap_or("not requested");
    let cancel_prompt = if dashboard.cancel_requested {
        "\n\nCancel requested — press y to confirm or n/Esc to continue"
    } else {
        ""
    };
    let text = format!(
        "{}\nView: {view_mode} ({pause_state})   [l] logs [p] pause [d] diagnostics [q] cancel{cancel_prompt}\n\nCurrent operation\n{}\n\nDiagnostics\n{}\n\nRecent output\n{}",
        title(dashboard),
        dashboard.current_operation.as_deref().unwrap_or("waiting"),
        diagnostics,
        output
    );
    frame.render_widget(
        Paragraph::new(crate::tui::layout::viewport(
            &text,
            area.height.saturating_sub(2),
            dashboard.scroll_offset(),
        ))
        .style(Style::default().fg(theme.text).bg(theme.panel))
        .block(Block::default().title("Execution").borders(Borders::ALL)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Configuration, DeployAction, DeployPlan};
    use crate::tui::theme::ThemeKind;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn execution_dashboard_renders_at_supported_sizes_and_themes() {
        let run = crate::domain::DeployRun::new(DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![crate::domain::Target::Windows],
            Configuration::Debug,
        ));
        for (width, height) in [(80, 24), (100, 30), (120, 30), (180, 45)] {
            for kind in [ThemeKind::Aurora, ThemeKind::Amber, ThemeKind::HighContrast] {
                let backend = TestBackend::new(width, height);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                let dashboard = ExecutionDashboard::new(run.clone());
                terminal
                    .draw(|frame| render(frame, frame.area(), &dashboard, Theme::for_kind(kind)))
                    .expect("execution render");
            }
        }
    }
}
