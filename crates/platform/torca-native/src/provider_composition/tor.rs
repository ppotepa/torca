use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use torca_pairing_coordinator::{PairingCoordinator, PairingRuntime};
use torca_pairing_driver::{
    PairingTransportRoute, PairingTransportRouteSource, RuntimePairingDriver,
};
use torca_radio_tor::TorRadioMediaSystemFactory;
use torca_rendezvous_client::RendezvousClient;
use torca_rendezvous_tor::SharedTorRelayTransport;
use torca_runtime::PairingDriver;
use torca_tor::{
    OwnedTorDriver, PeerListener, SharedTorEndpoint, TorBootstrapObserver, TorBootstrapStage,
};
use torca_transport_api::{
    CommissioningEvent, CommissioningObserver, CommissioningStage, TransportKind,
};
use torca_transport_tor::TorPeerTransportFactory;

use crate::composition::NativeCompositionError;
use crate::provider_composition::{
    ProviderComponents, ProviderPairingFactory, ProviderPairingInputs,
};
use crate::relay_probe::build_relay_probe;

/// Provider-owned resources required by the generic process runtime.
///
/// All Arti, onion listener and Tor relay details are contained in this
/// module. A future Iroh or WebRTC module returns the equivalent neutral
/// pieces without changing the native bootstrap flow.
/// Starts the complete Tor provider stack for one deployment-selected client.
pub(crate) fn compose(
    data_dir: PathBuf,
    relay_host: String,
    relay_port: u16,
    startup_timeout: std::time::Duration,
    now: torca_foundation::Timestamp,
    bootstrap_observer: CommissioningObserver,
) -> Result<ProviderComponents, NativeCompositionError> {
    let listener = PeerListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .map_err(|_| NativeCompositionError::new("bind local peer listener failed"))?;
    let endpoint = SharedTorEndpoint::default();
    let tor_observer: TorBootstrapObserver =
        std::sync::Arc::new(move |event| bootstrap_observer(map_bootstrap_event(event)));
    let lifecycle = OwnedTorDriver::bootstrap_observed(
        data_dir.join("tor"),
        listener.local_addr(),
        endpoint.clone(),
        startup_timeout,
        now,
        Some(tor_observer),
    )
    .map_err(|(error, diagnostic)| {
        NativeCompositionError::new(format!(
            "start Tor communication provider failed: {error}; diagnostic: {diagnostic}"
        ))
    })?;
    let client = lifecycle
        .client_handle()
        .ok_or_else(|| NativeCompositionError::new("Tor provider client is unavailable"))?;
    let pairing_endpoint = endpoint.clone();
    let relay_transport = SharedTorRelayTransport::new(client.clone(), relay_host, relay_port);
    Ok(ProviderComponents {
        provider: TransportKind::Tor,
        peer_transport_factory: Box::new(TorPeerTransportFactory::new(listener, client.clone())),
        pairing_factory: Box::new(TorPairingFactory {
            relay_transport: relay_transport.clone(),
            route_source: Box::new(move || {
                Ok(pairing_endpoint
                    .get()
                    .map(|onion| PairingTransportRoute::new("tor", onion.into_bytes())))
            }),
        }),
        rendezvous_probe: Some(build_relay_probe(relay_transport, RELAY_HEALTH_TIMEOUT)),
        radio_media_factory: Box::new(TorRadioMediaSystemFactory::new(client)),
        lifecycle: Box::new(lifecycle),
    })
}

const RELAY_HEALTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
const PAIRING_NETWORK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
const COMPILED_RENDEZVOUS_ENDPOINT: &str = match option_env!("TORCA_PROVIDER_ENDPOINT") {
    Some(value) => value,
    None => match option_env!("TORCA_RELAY_ENDPOINT") {
        Some(value) => value,
        None => "",
    },
};

/// Parses the Tor provider's managed onion rendezvous endpoint. This belongs
/// to Tor composition rather than platform services or generic runtime code.
pub(crate) fn compiled_rendezvous_endpoint() -> Result<(String, u16), NativeCompositionError> {
    parse_rendezvous_endpoint(COMPILED_RENDEZVOUS_ENDPOINT)
}

fn parse_rendezvous_endpoint(value: &str) -> Result<(String, u16), NativeCompositionError> {
    let (host, port) = value.rsplit_once(':').ok_or_else(|| {
        NativeCompositionError::new("Tor rendezvous endpoint must be host.onion:port")
    })?;
    let label = host.strip_suffix(".onion").ok_or_else(|| {
        NativeCompositionError::new("Tor rendezvous endpoint must use a v3 onion hostname")
    })?;
    if label.len() != 56 || !label.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'2'..=b'7')) {
        return Err(NativeCompositionError::new(
            "Tor rendezvous endpoint contains an invalid v3 onion hostname",
        ));
    }
    let port = port.parse::<u16>().map_err(|_| {
        NativeCompositionError::new("Tor rendezvous endpoint contains an invalid port")
    })?;
    if port == 0 {
        return Err(NativeCompositionError::new("Tor rendezvous endpoint port must be non-zero"));
    }
    Ok((host.to_owned(), port))
}

struct TorPairingFactory {
    relay_transport: SharedTorRelayTransport,
    route_source: Box<dyn PairingTransportRouteSource>,
}

impl ProviderPairingFactory for TorPairingFactory {
    fn build(
        self: Box<Self>,
        inputs: ProviderPairingInputs,
    ) -> Result<Box<dyn PairingDriver>, NativeCompositionError> {
        let rendezvous = RendezvousClient::new(self.relay_transport, PAIRING_NETWORK_TIMEOUT)
            .with_connectivity(inputs.connectivity);
        let coordinator =
            PairingCoordinator::new(rendezvous, torca_crypto::RustPairingCrypto::new());
        let mut runtime = PairingRuntime::new(
            coordinator,
            inputs.engine.clone(),
            inputs.approval,
            inputs.peer_secrets,
        );
        runtime
            .restore_active_sessions()
            .map_err(|_| NativeCompositionError::new("restore active pairing sessions failed"))?;
        Ok(Box::new(RuntimePairingDriver::new(runtime, inputs.engine, self.route_source)))
    }
}

fn map_bootstrap_event(event: torca_tor::TorBootstrapEvent) -> CommissioningEvent {
    CommissioningEvent {
        stage: match event.stage {
            TorBootstrapStage::Network => CommissioningStage::LocalRuntime,
            TorBootstrapStage::OnionService => CommissioningStage::IncomingReachability,
        },
        progress: event.progress,
        attempt: event.attempt,
        retry_after_ms: event.retry_after_ms,
        code: event.code.to_owned(),
        summary: event.summary,
    }
}

#[cfg(test)]
mod tests {
    use super::{map_bootstrap_event, parse_rendezvous_endpoint};
    use torca_tor::{TorBootstrapEvent, TorBootstrapStage};
    use torca_transport_api::CommissioningStage;

    #[test]
    fn maps_tor_private_types_to_generic_commissioning_stages() {
        let event = map_bootstrap_event(TorBootstrapEvent {
            stage: TorBootstrapStage::OnionService,
            progress: 25,
            attempt: 2,
            retry_after_ms: Some(100),
            code: "ONION_SERVICE_PUBLISHING",
            summary: "publishing".into(),
        });
        assert_eq!(event.stage, CommissioningStage::IncomingReachability);
        assert_eq!(event.code, "ONION_SERVICE_PUBLISHING");
    }

    #[test]
    fn tor_endpoint_validation_is_contained_in_tor_composition() {
        let host = format!("{}.onion", "a".repeat(56));
        assert_eq!(
            parse_rendezvous_endpoint(&format!("{host}:443")).expect("valid onion endpoint"),
            (host, 443)
        );
        assert!(parse_rendezvous_endpoint("example.com:443").is_err());
    }
}
