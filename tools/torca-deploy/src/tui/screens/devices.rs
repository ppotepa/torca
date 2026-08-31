use crate::domain::{CheckStatus, PreflightReport};

pub fn summary(report: &PreflightReport) -> String {
    summary_with_width(report, 96)
}

pub fn summary_with_width(report: &PreflightReport, width: usize) -> String {
    let detail_width = width.saturating_sub(8).max(20);
    report
        .checks
        .iter()
        .map(|check| {
            let marker = match check.status {
                CheckStatus::Pass => "[OK]",
                CheckStatus::Warn => "!",
                CheckStatus::Fail => "[X]",
                CheckStatus::Skipped => "[ ]",
            };
            match check.remediation.as_deref() {
                Some(remediation) => {
                    format!(
                        "{marker} {} - {}\n  remediation: {}",
                        crate::tui::layout::ellipsize(&check.name, detail_width / 3),
                        crate::tui::layout::ellipsize(&check.detail, detail_width),
                        crate::tui::layout::ellipsize(remediation, detail_width),
                    )
                }
                None => format!(
                    "{marker} {} - {}",
                    crate::tui::layout::ellipsize(&check.name, detail_width / 3),
                    crate::tui::layout::ellipsize(&check.detail, detail_width),
                ),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CheckStatus, PreflightCheck};

    #[test]
    fn preflight_summary_renders_remediation_for_blockers() {
        let report = PreflightReport {
            checks: vec![PreflightCheck {
                name: "Devices".into(),
                status: CheckStatus::Fail,
                detail: "selected Android target unavailable".into(),
                remediation: Some("connect or authorize the selected device".into()),
            }],
            can_execute: false,
        };
        let summary = summary(&report);
        assert!(summary.contains("remediation: connect or authorize the selected device"));
    }
}
