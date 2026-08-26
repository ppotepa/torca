use torca_transport_api::{TransportKind, TransportParseError};

#[cfg(not(any(feature = "provider-tor", feature = "provider-iroh", feature = "provider-webrtc",)))]
compile_error!("torca-native requires exactly one communication provider feature");

#[cfg(any(
    all(feature = "provider-tor", feature = "provider-iroh"),
    all(feature = "provider-tor", feature = "provider-webrtc"),
    all(feature = "provider-iroh", feature = "provider-webrtc"),
))]
compile_error!("torca-native supports exactly one communication provider feature");

#[cfg(feature = "provider-tor")]
const FEATURE_PROVIDER: &str = "tor";
#[cfg(feature = "provider-iroh")]
const FEATURE_PROVIDER: &str = "iroh";
#[cfg(feature = "provider-webrtc")]
const FEATURE_PROVIDER: &str = "webrtc";

pub(crate) fn compiled_provider() -> Result<TransportKind, TransportParseError> {
    // The Cargo feature is authoritative.  The environment value is retained
    // only as a consistency check for deployment metadata; it can never cause
    // an artifact to load a provider that was not linked into it.
    if let Some(configured) = option_env!("TORCA_COMMUNICATION_PROVIDER")
        && configured != FEATURE_PROVIDER
    {
        return Err(TransportParseError(format!(
            "provider '{configured}' does not match compiled feature '{FEATURE_PROVIDER}'"
        )));
    }
    TransportKind::from_wire(FEATURE_PROVIDER)
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
    fn compiled_provider_matches_feature() {
        let expected = if cfg!(feature = "provider-tor") {
            TransportKind::Tor
        } else if cfg!(feature = "provider-iroh") {
            TransportKind::Iroh
        } else {
            TransportKind::WebRtc
        };
        assert_eq!(compiled_provider().unwrap_or(TransportKind::Memory), expected);
    }

    #[test]
    fn provider_gate_matches_composition_readiness() {
        assert!(ensure_deployment_ready(TransportKind::Tor).is_ok());
        assert!(ensure_deployment_ready(TransportKind::Iroh).is_ok());
        assert!(ensure_deployment_ready(TransportKind::WebRtc).is_err());
    }
}
