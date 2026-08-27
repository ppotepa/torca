use crate::tui::model::FailureAction;
use crate::tui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

pub fn action_label(action: FailureAction) -> &'static str {
    match action {
        FailureAction::RetryFailedStage => "retry failed stage",
        FailureAction::EditPlan => "edit plan",
        FailureAction::Diagnostics => "open diagnostics",
        FailureAction::CollectLogs => "collect logs",
        FailureAction::Quit => "quit",
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, stage: &str, reason: &str, theme: Theme) {
    let text = format!(
        "Deployment failed\n\nStage: {stage}\nReason: {reason}\n\nSuggested actions:\n  r retry failed stage\n  p edit plan\n  d open diagnostics\n  l collect logs\n  q quit"
    );
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(theme.danger).bg(theme.panel))
            .block(Block::default().title("Failure").borders(Borders::ALL)),
        area,
    );
}
