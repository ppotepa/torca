use torca_transport_api::{TransportKind, TransportParseError};

const COMPILED_PROVIDER: &str = match option_env!("TORCA_COMMUNICATION_PROVIDER") {
    Some(value) => value,
    None => "tor",
};

pub(crate) fn compiled_provider() -> Result<TransportKind, TransportParseError> {
    TransportKind::from_wire(COMPILED_PROVIDER)
}

/// Rejects adapters whose complete native commissioning composition has not
/// been released yet. This prevents a build define from starting a partial
/// provider while the deployment manifest claims a different implementation.
pub(crate) fn ensure_deployment_ready(provider: TransportKind) -> Result<(), String> {
    if provider.deployment_profile().is_deployment_ready() {
        Ok(())
    } else {
        Err(format!("communication provider '{}' is not deployment-ready", provider.wire_value()))
    }
}

#[cfg(test)]
mod tests {
    use super::{compiled_provider, ensure_deployment_ready};
    use torca_transport_api::TransportKind;

    #[test]
    fn default_provider_is_tor() {
        assert_eq!(compiled_provider().unwrap_or(TransportKind::Memory), TransportKind::Tor);
    }

    #[test]
    fn provider_gate_matches_composition_readiness() {
        assert!(ensure_deployment_ready(TransportKind::Tor).is_ok());
        assert!(ensure_deployment_ready(TransportKind::Iroh).is_ok());
        assert!(ensure_deployment_ready(TransportKind::WebRtc).is_err());
    }
}
