//! Provider plugin metadata and routing boundary.
//!
//! Byte transports stay in `torca-transport-api`; this crate describes one
//! complete provider and owns the shared route/bootstrap contract used by
//! native composition.

use std::time::Duration;

use torca_foundation::ProviderId;
use torca_pairing_protocol::PairingBootstrapDescriptor;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderProfileDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceOption {
    Ensure,
    Restart,
}

impl MaintenanceOption {
    pub const fn wire_value(self) -> &'static str {
        match self {
            Self::Ensure => "ensure",
            Self::Restart => "restart",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Ensure => "Ensure",
            Self::Restart => "Restart",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub label: &'static str,
    pub description: &'static str,
    pub managed_service: bool,
    pub profiles: &'static [ProviderProfileDescriptor],
    pub maintenance: &'static [MaintenanceOption],
    pub endpoint_required: bool,
    pub warmup_stages: &'static [&'static str],
}

/// Stable deployment requirements for one communication provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderDeploymentProfile {
    pub label: &'static str,
    pub deployment_state: ProviderDeploymentState,
    pub commissioning_service: ProviderCommissioningService,
    pub pairing_bootstrap: PairingBootstrapMode,
    pub features: ProviderFeatures,
    pub startup_timeout: Duration,
    pub service_validation_timeout: Duration,
    pub local_ready_codes: &'static [&'static str],
    pub service_ready_codes: &'static [&'static str],
}

/// Product capabilities exposed by a selected provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProviderFeatures {
    pub pairing_qr: bool,
    pub pairing_full_link: bool,
    pub pairing_short_code: bool,
    pub incoming: bool,
    pub messages: bool,
    pub attachments: bool,
    pub radio: bool,
    pub direct_path: bool,
}

impl ProviderFeatures {
    pub const IROH: Self = Self {
        pairing_qr: true,
        pairing_full_link: true,
        pairing_short_code: false,
        incoming: true,
        messages: true,
        attachments: true,
        radio: true,
        direct_path: true,
    };
    pub const MEMORY: Self = Self {
        pairing_qr: true,
        pairing_full_link: true,
        pairing_short_code: true,
        incoming: true,
        messages: true,
        attachments: true,
        radio: false,
        direct_path: true,
    };
}

impl ProviderDeploymentProfile {
    pub const fn is_deployment_ready(self) -> bool {
        matches!(self.deployment_state, ProviderDeploymentState::Validated)
    }

    pub const fn requires_service_readiness(self) -> bool {
        !matches!(self.commissioning_service, ProviderCommissioningService::None)
    }

    pub fn endpoint_is_valid(self, endpoint: &str) -> bool {
        if endpoint.is_empty() || endpoint.len() > 2048 || endpoint.chars().any(char::is_whitespace)
        {
            return false;
        }
        match self.commissioning_service {
            ProviderCommissioningService::ManagedRendezvous => false,
            ProviderCommissioningService::ExternalSignaling => {
                endpoint.starts_with("https://") || endpoint.starts_with("wss://")
            }
            ProviderCommissioningService::ExternalRendezvous => {
                endpoint.contains(':') || endpoint.starts_with("https://")
            }
            ProviderCommissioningService::None => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderDeploymentState {
    Hidden,
    Experimental,
    Validated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingBootstrapMode {
    ManagedSessionService,
    DirectQr,
    ExternalSignaling,
    TestMemory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderCommissioningService {
    None,
    ManagedRendezvous,
    ExternalRendezvous,
    ExternalSignaling,
}

impl ProviderCommissioningService {
    pub const fn requires_endpoint(self) -> bool {
        matches!(self, Self::ManagedRendezvous | Self::ExternalRendezvous | Self::ExternalSignaling)
    }

    pub const fn is_managed(self) -> bool {
        matches!(self, Self::ManagedRendezvous)
    }
}

/// Provider-neutral availability of the local route used for new sessions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRouteState {
    Fresh,
    Stale,
    Unavailable,
}

impl ProviderRouteState {
    pub const fn is_fresh(self) -> bool {
        matches!(self, Self::Fresh)
    }
}

/// Metadata catalog used by compatibility adapters and provider plugins.
/// The provider identifier, rather than a transport enum, is the lookup key.
pub fn built_in_descriptor(id: &ProviderId) -> Option<ProviderDescriptor> {
    let descriptor = match id.as_str() {
        "iroh" => ProviderDescriptor {
            id: id.clone(),
            label: "Iroh (QUIC)",
            description: "Direct QUIC provider with optional discovery and relay fallback.",
            managed_service: false,
            profiles: &[
                ProviderProfileDescriptor {
                    id: "always",
                    label: "Always reachable",
                    description: "Discovery and relay fallback.",
                },
                ProviderProfileDescriptor {
                    id: "direct",
                    label: "Direct only",
                    description: "Direct paths only.",
                },
                ProviderProfileDescriptor {
                    id: "local",
                    label: "Local only",
                    description: "Loopback or lab use.",
                },
            ],
            maintenance: &[],
            endpoint_required: false,
            warmup_stages: &["start local endpoint"],
        },
        "memory" => ProviderDescriptor {
            id: id.clone(),
            label: "Memory (test)",
            description: "In-process test provider.",
            managed_service: false,
            profiles: &[],
            maintenance: &[],
            endpoint_required: false,
            warmup_stages: &["start local endpoint"],
        },
        _ => return None,
    };
    Some(descriptor)
}

pub fn built_in_deployment_profile(id: &ProviderId) -> Option<ProviderDeploymentProfile> {
    let profile = match id.as_str() {
        "iroh" => ProviderDeploymentProfile {
            label: "Iroh (QUIC)",
            deployment_state: ProviderDeploymentState::Validated,
            commissioning_service: ProviderCommissioningService::None,
            pairing_bootstrap: PairingBootstrapMode::DirectQr,
            features: ProviderFeatures::IROH,
            startup_timeout: Duration::from_secs(45),
            service_validation_timeout: Duration::from_secs(45),
            local_ready_codes: &["LOCAL_READY"],
            service_ready_codes: &["COMMUNICATION_READY", "NETWORK_READY"],
        },
        "memory" => ProviderDeploymentProfile {
            label: "Memory (test)",
            deployment_state: ProviderDeploymentState::Hidden,
            commissioning_service: ProviderCommissioningService::None,
            pairing_bootstrap: PairingBootstrapMode::TestMemory,
            features: ProviderFeatures::MEMORY,
            startup_timeout: Duration::from_secs(5),
            service_validation_timeout: Duration::from_secs(5),
            local_ready_codes: &["LOCAL_READY", "COMMUNICATION_READY"],
            service_ready_codes: &["COMMUNICATION_READY", "NETWORK_READY"],
        },
        _ => return None,
    };
    Some(profile)
}

/// Provider-owned, opaque route advertised after authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRoute {
    pub provider: ProviderId,
    pub generation: u64,
    pub endpoint: Vec<u8>,
}

impl ProviderRoute {
    pub fn new(provider: ProviderId, generation: u64, endpoint: Vec<u8>) -> Option<Self> {
        (!endpoint.is_empty() && endpoint.len() <= 8 * 1024).then_some(Self {
            provider,
            generation,
            endpoint,
        })
    }
}

/// Redaction-safe failure at the provider route owner boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRouteError {
    Unavailable,
    Stale,
    Invalid,
    Provider,
}

/// Single owner for peer routes and pairing bootstrap material.
pub trait ProviderRouting: Send + Sync {
    fn route_state(&self) -> ProviderRouteState;
    fn local_route(&self) -> Result<Option<ProviderRoute>, ProviderRouteError>;
    fn pairing_bootstrap(&self) -> Result<Option<PairingBootstrapDescriptor>, ProviderRouteError>;
}

#[cfg(test)]
mod tests {
    use super::{built_in_deployment_profile, built_in_descriptor};
    use torca_foundation::ProviderId;

    #[test]
    fn built_in_metadata_is_keyed_by_validated_provider_id() {
        let id = ProviderId::new("iroh").expect("provider id");
        assert_eq!(built_in_descriptor(&id).expect("descriptor").id, id);
        assert!(built_in_deployment_profile(&id).expect("profile").features.direct_path);
    }
}
