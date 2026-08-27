//! Application-level TUI entry points.
//!
//! The terminal lifecycle and execution dashboard are kept behind this
//! module boundary so callers do not need to know how Ratatui is wired.

use std::io;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crossterm::{
    event::{self, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use super::input::InputGuard;
use super::screens;
use super::theme::{Theme, ThemeKind};
use crate::domain::{DeployPlan, DeployRun};
use crate::executor::{
    CancellationToken, DeployError, DeployExecutor, DeployProgress, ExecutionMode,
};

/// Execute a normalized plan in the same terminal lifecycle as the wizard.
pub fn execute_plan_with_dashboard(
    deployment: DeployExecutor,
    plan: DeployPlan,
    theme_kind: ThemeKind,
    no_color: bool,
) -> Result<DeployRun, Box<dyn std::error::Error>> {
    let requested_plan = plan.clone();
    let plan = crate::planner::normalize(plan);
    let normalization_diff = requested_plan.normalized_diff();
    let report = deployment.preflight(&plan);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = if run_preflight_screen(&mut terminal, &report, theme_kind, no_color)? {
        if screens::review::confirm_with_diff(
            &mut terminal,
            &plan,
            &report,
            &normalization_diff,
            theme_kind,
            no_color,
        )? {
            let initial_run = deployment.create_run(plan)?;
            run_execution_dashboard(&mut terminal, deployment, initial_run, theme_kind, no_color)
        } else {
            Err("deployment cancelled during review".into())
        }
    } else {
        Err(format!("preflight blocked execution: {}", screens::preflight::text(&report)).into())
    };
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_preflight_screen(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    report: &crate::domain::PreflightReport,
    theme_kind: ThemeKind,
    no_color: bool,
) -> io::Result<bool> {
    let theme = if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
    let mut input = InputGuard::default();
    let mut scroll = 0_usize;
    loop {
        terminal.draw(|frame| {
            screens::preflight::render_with_scroll(frame, frame.area(), report, theme, scroll);
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Some(key) = input.read()? {
                return Ok(match key {
                    KeyCode::Char('q') | KeyCode::Esc => false,
                    KeyCode::Enter | KeyCode::Char('y') => report.can_execute,
                    KeyCode::PageDown => {
                        scroll = scroll.saturating_add(5);
                        continue;
                    }
                    KeyCode::PageUp => {
                        scroll = scroll.saturating_sub(5);
                        continue;
                    }
                    _ => continue,
                });
            }
        }
    }
}

fn run_execution_dashboard(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    deployment: DeployExecutor,
    mut run: DeployRun,
    mut theme_kind: ThemeKind,
    no_color: bool,
) -> Result<DeployRun, Box<dyn std::error::Error>> {
    let mut theme = if no_color { Theme::monochrome() } else { Theme::for_kind(theme_kind) };
    let mut input = InputGuard::default();
    loop {
        let (progress_tx, progress_rx) = mpsc::channel::<DeployProgress>();
        let (result_tx, result_rx) = mpsc::channel::<Result<DeployRun, DeployError>>();
        let (diagnostics_tx, diagnostics_rx) = mpsc::channel::<Result<String, String>>();
        let cancellation = CancellationToken::default();
        let worker_deployment = deployment.clone();
        let worker_cancel = cancellation.clone();
        let worker_run = run.clone();
        thread::spawn(move || {
            let sink = Arc::new(move |progress: DeployProgress| {
                let _ = progress_tx.send(progress);
            });
            let result = worker_deployment.with_progress_sink(sink).execute_with_cancel(
                worker_run,
                ExecutionMode::Execute,
                &worker_cancel,
            );
            let _ = result_tx.send(result);
        });

        let mut dashboard = super::model::ExecutionDashboard::new(run.clone());
        let mut diagnostics_in_flight = false;
        let mut failure: Option<String> = None;
        loop {
            while let Ok(progress) = progress_rx.try_recv() {
                dashboard.receive_progress(progress);
            }
            if let Ok(result) = diagnostics_rx.try_recv() {
                diagnostics_in_flight = false;
                dashboard.set_diagnostics_status(match result {
                    Ok(summary) => summary,
                    Err(error) => error,
                });
            }
            if let Ok(result) = result_rx.try_recv() {
                match result {
                    Ok(completed) => return Ok(completed),
                    Err(error) => {
                        failure = Some(error.to_string());
                        run = deployment
                            .resume(ExecutionMode::DryRun)
                            .unwrap_or_else(|_| dashboard.run.clone());
                        break;
                    }
                }
            }
            terminal.draw(|frame| {
                let area = frame.area();
                if let Some(reason) = failure.as_deref() {
                    screens::failure::render(
                        frame,
                        area,
                        &format!("{:?}", run.stage),
                        reason,
                        theme,
                    );
                } else {
                    screens::execution::render(frame, area, &dashboard, theme);
                }
            })?;
            if event::poll(Duration::from_millis(100))? {
                if let Some(key) = input.read()? {
                    match key {
                        KeyCode::Char('l') => dashboard.toggle_raw_logs(),
                        KeyCode::Char('p') => dashboard.toggle_pause(),
                        KeyCode::PageUp => dashboard.scroll_up(),
                        KeyCode::PageDown => dashboard.scroll_down(),
                        KeyCode::Char('d') if !diagnostics_in_flight => {
                            diagnostics_in_flight = true;
                            dashboard.set_diagnostics_status("collecting...");
                            let diagnostics_deployment = deployment.clone();
                            let diagnostics_run = dashboard.run.clone();
                            let diagnostics_result = diagnostics_tx.clone();
                            thread::spawn(move || {
                                let result = diagnostics_deployment
                                    .collect_diagnostics(&diagnostics_run)
                                    .map(|report| report.summary())
                                    .map_err(|error| error.to_string());
                                let _ = diagnostics_result.send(result);
                            });
                        }
                        KeyCode::Char('q') => dashboard.request_cancel(),
                        KeyCode::Char('y') if dashboard.cancel_requested => {
                            cancellation.cancel();
                        }
                        KeyCode::Char('n') | KeyCode::Esc if dashboard.cancel_requested => {
                            dashboard.clear_cancel_request();
                        }
                        _ => {}
                    }
                }
            }
        }

        loop {
            terminal.draw(|frame| {
                screens::failure::render(
                    frame,
                    frame.area(),
                    &format!("{:?}", run.stage),
                    failure.as_deref().unwrap_or("unknown failure"),
                    theme,
                );
            })?;
            if event::poll(Duration::from_millis(100))? {
                if let Some(key) = input.read()? {
                    match key {
                        KeyCode::Char('r') => break,
                        KeyCode::Char('d') => match deployment.collect_diagnostics(&run) {
                            Ok(report) => {
                                failure =
                                    Some(format!("diagnostics collected: {}", report.summary()));
                            }
                            Err(error) => {
                                failure = Some(format!("diagnostics failed: {error}"));
                            }
                        },
                        KeyCode::Char('l') => match deployment.collect_logs(&run) {
                            Ok(report) => {
                                failure = Some(format!("logs collected: {}", report.summary()));
                            }
                            Err(error) => {
                                failure = Some(format!("log collection failed: {error}"));
                            }
                        },
                        KeyCode::Char('q') | KeyCode::Esc => {
                            return Err(failure
                                .unwrap_or_else(|| "deployment failed".into())
                                .into());
                        }
                        KeyCode::Char('t') => {
                            theme_kind = match theme_kind {
                                ThemeKind::Aurora => ThemeKind::Amber,
                                ThemeKind::Amber => ThemeKind::HighContrast,
                                ThemeKind::HighContrast => ThemeKind::Aurora,
                            };
                            theme = if no_color {
                                Theme::monochrome()
                            } else {
                                Theme::for_kind(theme_kind)
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
        let _ = failure.take();
        run = deployment.resume(ExecutionMode::DryRun)?;
    }
}
