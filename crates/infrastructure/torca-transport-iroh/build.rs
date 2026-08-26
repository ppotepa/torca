fn main() {
    for key in [
        "TORCA_IROH_PROFILE",
        "TORCA_IROH_RELAY_URLS",
        "TORCA_IROH_PKARR_URL",
        "TORCA_IROH_DISABLE_RELAY",
        "TORCA_IROH_DISABLE_DISCOVERY",
        "TORCA_IROH_LOCAL_ONLY",
        "TORCA_IROH_RUNTIME_THREADS",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
        // These values are routing configuration, not credentials. Reject
        // line breaks so an environment value cannot inject another cargo
        // directive into the generated build metadata.
        let value = std::env::var(key).unwrap_or_default();
        if !value.contains('\r') && !value.contains('\n') {
            // Emit an explicit empty value as well. This makes a packaged
            // artifact deterministic: an absent build-time relay setting
            // cannot later be filled from a different host process
            // environment after installation.
            println!("cargo:rustc-env={key}={value}");
        }
    }
}
