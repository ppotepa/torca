//! Canonical Android application identity selected for a deployment process.

use std::env;

/// SOAK2 is selected explicitly by the launcher or implicitly when the
/// deployment library is hosted by a soak executable. The latter keeps
/// `cargo run -p torca-soak` a complete, single entry point.
pub fn is_soak() -> bool {
    env::var("TORCA_SOAK_FLAVOR")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        || env::current_exe()
            .ok()
            .and_then(|path| path.file_stem().map(|value| value.to_string_lossy().to_string()))
            .is_some_and(|name| name.starts_with("torca-soak") || name == "torca-battery-soak-tui")
}

pub fn package() -> &'static str {
    if is_soak() { "com.torca.torca_app.soak" } else { "com.torca.torca_app" }
}

pub fn activity() -> &'static str {
    if is_soak() {
        "com.torca.torca_app.soak/com.torca.app.MainActivity"
    } else {
        "com.torca.torca_app/com.torca.app.MainActivity"
    }
}

pub fn logs_root() -> &'static str {
    if is_soak() {
        "/sdcard/Android/data/com.torca.torca_app.soak/files/torca/logs"
    } else {
        "/sdcard/Android/data/com.torca.torca_app/files/torca/logs"
    }
}
