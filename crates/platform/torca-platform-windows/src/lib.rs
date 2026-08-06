#![deny(unsafe_op_in_unsafe_fn)]
//! Windows-specific Torca platform integrations.
//!
//! Unsafe Win32 calls are isolated in this crate. Domain and application crates remain safe Rust.

#[cfg(windows)]
mod dpapi;

#[cfg(windows)]
pub use dpapi::DpapiFileSecretStore;

/// Marker available on non-Windows build hosts so the workspace remains cross-platform checkable.
#[cfg(not(windows))]
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsPlatformUnavailable;
