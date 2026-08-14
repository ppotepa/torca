//! Typed deployment planning, durable checkpoints and command execution for Torca.
//!
//! The crate intentionally owns deployment state while delegating platform-specific
//! commands to narrow adapters. This makes an interrupted deployment resumable and
//! keeps the Ratatui interface and non-interactive CLI on the same execution path.

pub mod build;
pub mod cli;
pub mod data;
pub mod devices;
pub mod diagnostics;
pub mod domain;
pub mod executor;
pub mod install;
pub mod launch;
pub mod manifests;
pub mod paths;
pub mod persistence;
pub mod planner;
pub mod process;
pub mod relay;
pub mod tui;
pub mod windows_client;

pub use domain::{DeployPlan, DeployRun, DeployStage, PlanError};
pub use executor::{DeployExecutor, ExecutionMode};
