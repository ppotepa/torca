use crate::domain::PreflightReport;
use crate::tui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

pub fn text(report: &PreflightReport) -> String {
    text_with_width(report, 96)
}

pub fn text_with_width(report: &PreflightReport, width: usize) -> String {
    let checks = crate::tui::screens::devices::summary_with_width(report, width);
    format!("Device preflight\n\n{checks}\n\nCan execute: {}", report.can_execute)
}

pub fn render(frame: &mut Frame<'_>, area: Rect, report: &PreflightReport, theme: Theme) {
    render_with_scroll(frame, area, report, theme, 0);
}

pub fn render_with_scroll(
    frame: &mut Frame<'_>,
    area: Rect,
    report: &PreflightReport,
    theme: Theme,
    scroll: usize,
) {
    frame.render_widget(
        Paragraph::new(crate::tui::layout::viewport(
            &text_with_width(report, usize::from(area.width.saturating_sub(2))),
            area.height.saturating_sub(2),
            scroll,
        ))
        .style(Style::default().fg(theme.text).bg(theme.panel))
        .block(Block::default().title("Device preflight").borders(Borders::ALL)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CheckStatus, PreflightCheck};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn preflight_renders_and_scrolls_at_supported_sizes() {
        let report = PreflightReport {
            checks: (0..12)
                .map(|index| PreflightCheck {
                    name: format!("check-{index}"),
                    status: CheckStatus::Warn,
                    detail: "connect the device and retry discovery".into(),
                    remediation: Some("unlock or authorize the device".into()),
                })
                .collect(),
            can_execute: false,
        };
        for (width, height) in [(80, 24), (100, 30), (120, 30), (180, 45)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| {
                    render_with_scroll(frame, frame.area(), &report, Theme::high_contrast(), 5);
                })
                .expect("preflight render");
        }
        assert!(text(&report).contains("Can execute: false"));
    }
}
