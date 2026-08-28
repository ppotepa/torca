use torca_foundation::ProviderId;

pub(crate) fn compiled_provider() -> Result<ProviderId, torca_foundation::InvalidProviderId> {
    ProviderId::new("iroh")
}

/// Rejects adapters whose complete native commissioning composition has not
/// been released yet. This prevents a build define from starting a partial
/// provider while the deployment manifest claims a different implementation.
pub(crate) fn ensure_deployment_ready(provider: &ProviderId) -> Result<(), String> {
    if provider.as_str() == "iroh" {
        Ok(())
    } else {
        Err(format!("unsupported communication provider '{provider}'"))
    }
}

#[cfg(test)]
mod tests {
    use super::{compiled_provider, ensure_deployment_ready};
    use torca_foundation::ProviderId;
    #[test]
    fn compiled_provider_matches_feature() {
        assert_eq!(compiled_provider().expect("Iroh is built in").as_str(), "iroh");
    }

    #[test]
    fn provider_gate_matches_composition_readiness() {
        assert!(ensure_deployment_ready(&ProviderId::new("iroh").expect("provider")).is_ok());
        assert!(ensure_deployment_ready(&ProviderId::new("test").expect("provider")).is_err());
    }
}
