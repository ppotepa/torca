//! Provider conformance contract for the production Iroh provider and the
//! deterministic Memory test double.

use torca_foundation::ProviderId;
use torca_transport_api::{TransportPath, TransportTopology};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConformanceProvider {
    Memory,
    Iroh,
}

impl ConformanceProvider {
    #[allow(clippy::missing_panics_doc)]
    pub fn provider_id(self) -> ProviderId {
        match self {
            Self::Memory => ProviderId::new("memory").expect("static provider id"),
            Self::Iroh => ProviderId::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceRoute {
    pub path: TransportPath,
    pub persisted: bool,
}

pub fn persisted_route(
    provider: ConformanceProvider,
    topology: TransportTopology,
) -> ConformanceRoute {
    ConformanceRoute {
        path: TransportPath { provider: provider.provider_id(), topology },
        persisted: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_and_iroh_share_the_same_opaque_route_contract() {
        let memory = persisted_route(ConformanceProvider::Memory, TransportTopology::Direct);
        let iroh = persisted_route(ConformanceProvider::Iroh, TransportTopology::Direct);
        assert!(memory.persisted && iroh.persisted);
        assert_eq!(memory.path.topology, iroh.path.topology);
        assert_eq!(iroh.path.provider, ProviderId::default());
    }

    #[test]
    fn relay_topology_is_provider_neutral() {
        let route = persisted_route(ConformanceProvider::Iroh, TransportTopology::Relay);
        assert_eq!(route.path.topology, TransportTopology::Relay);
    }
}
