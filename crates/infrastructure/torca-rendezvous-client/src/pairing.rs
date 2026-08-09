use torca_pairing::PairingCode;
use torca_pairing_coordinator::{
    PairingCoordinatorError, PairingRendezvousPort, PairingSideToken, PairingSlotCapability,
    PairingSlotId,
};
use torca_relay_protocol::{RelayCode, RelaySideToken, RelaySlotCapability};

use crate::{RelayTransport, RendezvousClient};

impl<T: RelayTransport> PairingRendezvousPort for RendezvousClient<T> {
    fn open(
        &mut self,
        code: &PairingCode,
        expires_at: torca_foundation::Timestamp,
        creator_blob: Vec<u8>,
        capability: PairingSlotCapability,
        creator_token: PairingSideToken,
    ) -> Result<PairingSlotId, PairingCoordinatorError> {
        let relay_code =
            RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
        RendezvousClient::open(
            self,
            relay_code,
            expires_at,
            creator_blob,
            RelaySlotCapability(capability.0),
            RelaySideToken(creator_token.0),
        )
        .map(|slot| PairingSlotId(slot.0))
        .map_err(|_| PairingCoordinatorError::Rendezvous)
    }

    fn join(
        &mut self,
        code: &PairingCode,
        joiner_blob: Vec<u8>,
        joiner_token: PairingSideToken,
    ) -> Result<(PairingSlotId, Vec<u8>), PairingCoordinatorError> {
        let relay_code =
            RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
        RendezvousClient::join(self, relay_code, joiner_blob, RelaySideToken(joiner_token.0))
            .map(|(slot, creator_blob)| (PairingSlotId(slot.0), creator_blob))
            .map_err(|_| PairingCoordinatorError::Rendezvous)
    }

    fn push(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        blob: Vec<u8>,
    ) -> Result<(), PairingCoordinatorError> {
        RendezvousClient::push(
            self,
            torca_relay_protocol::RelaySlotId(slot.0),
            RelaySideToken(token.0),
            blob,
        )
        .map_err(|_| PairingCoordinatorError::Rendezvous)
    }

    fn poll(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
    ) -> Result<Vec<Vec<u8>>, PairingCoordinatorError> {
        RendezvousClient::poll(
            self,
            torca_relay_protocol::RelaySlotId(slot.0),
            RelaySideToken(token.0),
        )
        .map_err(|_| PairingCoordinatorError::Rendezvous)
    }

    fn close(
        &mut self,
        slot: PairingSlotId,
        capability: PairingSlotCapability,
    ) -> Result<(), PairingCoordinatorError> {
        RendezvousClient::close(
            self,
            torca_relay_protocol::RelaySlotId(slot.0),
            RelaySlotCapability(capability.0),
        )
        .map_err(|_| PairingCoordinatorError::Rendezvous)
    }
}
