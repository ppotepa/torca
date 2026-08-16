use core::fmt;
use std::collections::BTreeMap;

use torca_foundation::{OpaqueId, Timestamp};
use torca_pairing::{PairingCode, PairingRole, PairingSessionId};
use torca_pairing_protocol::PairingEnvelope;

include!("core/model_ports.rs");

impl<R, C> PairingCoordinator<R, C>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
{
    include!("core/coordinator_methods.rs");
}

include!("core/codec.rs");
