use std::sync::{Arc, Mutex};
use std::time::Duration;

use torca_foundation::ErrorCode;
use torca_rendezvous_client::{RelayTransportFailureKind, SharedTorRelayTransport};
use torca_runtime::{RelayProbe, RelayServiceInfo};

pub(crate) fn build_relay_probe(
    transport: SharedTorRelayTransport,
    timeout: Duration,
) -> Arc<dyn RelayProbe> {
    Arc::new(TorRelayProbe { transport, timeout, info: Mutex::new(None) })
}

struct TorRelayProbe {
    transport: SharedTorRelayTransport,
    timeout: Duration,
    info: Mutex<Option<RelayServiceInfo>>,
}

impl RelayProbe for TorRelayProbe {
    fn probe(&self) -> Result<(), ErrorCode> {
        // Pairing and health checks intentionally share one durable relay
        // transport. `relay_info` owns reconnect serialization, including the
        // first connection for a fresh profile.
        self.transport
            .relay_info(self.timeout)
            .map(|info| {
                if let Ok(mut current) = self.info.lock() {
                    *current = Some(RelayServiceInfo {
                        product_version: info.product_version,
                        build_id: info.build_id,
                        source_commit: info.source_commit,
                        protocol_version: info.protocol_version,
                    });
                }
            })
            .map_err(|error| ErrorCode::new(error_code(error.kind)))
    }

    fn service_info(&self) -> Option<RelayServiceInfo> {
        self.info.lock().ok().and_then(|value| value.clone())
    }
}

const fn error_code(kind: RelayTransportFailureKind) -> &'static str {
    match kind {
        RelayTransportFailureKind::Busy => "relay.connection_busy",
        RelayTransportFailureKind::Unavailable => "relay.connection_unavailable",
        RelayTransportFailureKind::Timeout => "relay.request_timeout",
        RelayTransportFailureKind::Disconnected => "relay.connection_disconnected",
        RelayTransportFailureKind::InvalidResponse => "relay.health_response_invalid",
    }
}

#[cfg(test)]
mod tests {
    use super::error_code;
    use torca_rendezvous_client::RelayTransportFailureKind;

    #[test]
    fn relay_failure_codes_are_stable_and_specific() {
        assert_eq!(error_code(RelayTransportFailureKind::Busy), "relay.connection_busy");
        assert_eq!(error_code(RelayTransportFailureKind::Timeout), "relay.request_timeout");
        assert_eq!(
            error_code(RelayTransportFailureKind::InvalidResponse),
            "relay.health_response_invalid"
        );
    }
}
