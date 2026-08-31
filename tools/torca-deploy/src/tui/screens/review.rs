use std::io;

use crossterm::event::KeyCode;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

use crate::domain::{DeployPlan, PlanDiff, PreflightReport, PrivacyPolicy, StepDisposition};
use crate::tui::{
    input::InputGuard,
    theme::{Theme, ThemeKind},
};

pub fn confirm(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    plan: &DeployPlan,
    report: &PreflightReport,
    theme_kind: ThemeKind,
    no_color: bool,
) -> io::Result<bool> {
    confirm_with_diff(terminal, plan, report, &plan.normalized_diff(), theme_kind, no_color)
}

pub fn confirm_with_diff(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    plan: &DeployPlan,
    report: &PreflightReport,
    diff: &PlanDiff,
    theme_kind: ThemeKind,
    no_color: bool,
) -> io::Result<bool> {
    let theme = if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
    let mut input = InputGuard::default();
    let mut scroll = 0_usize;
    loop {
        terminal.draw(|frame| {
            frame.render_widget(
                Paragraph::new(crate::tui::layout::viewport(
                    &text_with_diff(plan, report, diff),
                    frame.area().height.saturating_sub(2),
                    scroll,
                ))
                .style(Style::default().fg(theme.text).bg(theme.panel))
                .block(Block::default().title("Confirm deployment").borders(Borders::ALL)),
                frame.area(),
            );
        })?;
        if let Some(key) = input.read()? {
            match key {
                KeyCode::Char('y') | KeyCode::Enter => return Ok(true),
                KeyCode::Char('n') | KeyCode::Esc => return Ok(false),
                KeyCode::PageDown => scroll = scroll.saturating_add(5),
                KeyCode::PageUp => scroll = scroll.saturating_sub(5),
                _ => {}
            }
        }
    }
}

pub fn text(plan: &DeployPlan, report: &PreflightReport) -> String {
    text_with_diff(plan, report, &plan.normalized_diff())
}

pub fn text_with_diff(plan: &DeployPlan, report: &PreflightReport, diff: &PlanDiff) -> String {
    let targets = plan.targets.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    let devices = report
        .checks
        .iter()
        .find(|check| check.name == "Devices")
        .map(|check| check.detail.as_str())
        .unwrap_or("not checked");
    let steps = plan
        .planned_steps()
        .into_iter()
        .map(|step| {
            let marker = match step.disposition {
                StepDisposition::Execute => "[OK]",
                StepDisposition::Reuse => "[->]",
                StepDisposition::Skip => "[ ]",
                StepDisposition::Blocked => "[X]",
            };
            format!("{marker} {} - {}", step.label, step.reason)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let normalization = if diff.changes.is_empty() {
        String::new()
    } else {
        format!("\n\nAutomatic normalization\n{}", diff.changes.join("\n"))
    };
    format!(
        "Action: {}\nTargets: {}\nDevices: {}\nBuild: {}\nProvider: Iroh\nIroh profile: {}\nPrivacy: {}\n\nSteps\n{}{}\n\nPress y to execute, n/Esc to cancel",
        plan.action,
        if targets.is_empty() { "none" } else { &targets },
        devices,
        plan.configuration,
        plan.provider_profile.as_deref().unwrap_or("default"),
        privacy_label(plan.privacy),
        steps,
        normalization
    )
}

fn privacy_label(policy: PrivacyPolicy) -> &'static str {
    match policy {
        PrivacyPolicy::Strict => "Strict (block screenshots/recording)",
        PrivacyPolicy::AllowCapture => "Allow screenshots/recording",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CheckStatus, Configuration, DeployAction, PreflightCheck, Target};

    #[test]
    fn review_text_contains_actual_preflight_device_detail() {
        let plan = DeployPlan::normal(
            DeployAction::RedeployCurrent,
            vec![Target::Windows, Target::Android],
            Configuration::Debug,
        );
        let report = PreflightReport {
            checks: vec![PreflightCheck {
                name: "Devices".into(),
                status: CheckStatus::Pass,
                detail: "windows: DESKTOP-Q17P337, android: 85Z5AIGU79XSLZMZ".into(),
                remediation: None,
            }],
            can_execute: true,
        };
        let rendered = text(&plan, &report);
        assert!(rendered.contains("DESKTOP-Q17P337"));
        assert!(rendered.contains("85Z5AIGU79XSLZMZ"));
    }

    #[test]
    fn review_text_shows_automatic_normalization() {
        let requested = DeployPlan::normal(
            DeployAction::RunInstalled,
            vec![Target::Windows],
            Configuration::Debug,
        );
        let normalized = requested.clone().normalized();
        let report = PreflightReport { checks: Vec::new(), can_execute: true };
        let rendered = text_with_diff(&normalized, &report, &requested.normalized_diff());
        assert!(rendered.contains("Automatic normalization"));
        assert!(rendered.contains("client_build"));
    }
}
