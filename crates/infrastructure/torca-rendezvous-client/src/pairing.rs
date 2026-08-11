use torca_pairing::PairingCode;
use torca_pairing_coordinator::{
    PairingCoordinatorError, PairingRelayDelivery, PairingRendezvousPort, PairingSideToken,
    PairingSlotCapability, PairingSlotId,
};
use torca_relay_protocol::{RelayCode, RelaySequence, RelaySideToken, RelaySlotCapability};

use crate::{RelayTransport, RendezvousClient};

impl<T: RelayTransport> PairingRendezvousPort for RendezvousClient<T> {
    fn network_changed(&mut self) {
        RendezvousClient::network_changed(self);
    }
    fn open(
        &mut self,
        code: &PairingCode,
        expires_at: torca_foundation::Timestamp,
        creator_blob: Vec<u8>,
        capability: PairingSlotCapability,
        creator_token: PairingSideToken,
        ticket: [u8; 16],
    ) -> Result<(PairingSlotId, torca_foundation::Timestamp), PairingCoordinatorError> {
        let relay_code =
            RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
        RendezvousClient::open(
            self,
            relay_code,
            expires_at,
            creator_blob,
            RelaySlotCapability(capability.0),
            RelaySideToken(creator_token.0),
            ticket,
        )
        .map(|(slot, expires_at)| (PairingSlotId(slot.0), expires_at))
        .map_err(|_| PairingCoordinatorError::Rendezvous)
    }

    fn join(
        &mut self,
        code: &PairingCode,
        joiner_blob: Vec<u8>,
        joiner_token: PairingSideToken,
        ticket: Option<[u8; 16]>,
    ) -> Result<(PairingSlotId, torca_foundation::Timestamp, Vec<u8>), PairingCoordinatorError>
    {
        let relay_code =
            RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
        RendezvousClient::join(
            self,
            relay_code,
            joiner_blob,
            RelaySideToken(joiner_token.0),
            ticket,
        )
        .map(|(slot, expires_at, creator_blob)| (PairingSlotId(slot.0), expires_at, creator_blob))
        .map_err(|_| PairingCoordinatorError::Rendezvous)
    }

    fn push(
        &mut self,
        message_id: torca_foundation::OpaqueId,
        slot: PairingSlotId,
        token: PairingSideToken,
        blob: Vec<u8>,
    ) -> Result<(), PairingCoordinatorError> {
        RendezvousClient::push(
            self,
            message_id,
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
        after: u64,
    ) -> Result<Vec<PairingRelayDelivery>, PairingCoordinatorError> {
        RendezvousClient::poll(
            self,
            torca_relay_protocol::RelaySlotId(slot.0),
            RelaySideToken(token.0),
            RelaySequence(after),
        )
        .map(|deliveries| {
            deliveries
                .into_iter()
                .map(|delivery| PairingRelayDelivery {
                    sequence: delivery.sequence.0,
                    blob: delivery.blob,
                })
                .collect()
        })
        .map_err(|_| PairingCoordinatorError::Rendezvous)
    }

    fn ack(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        up_to: u64,
    ) -> Result<(), PairingCoordinatorError> {
        RendezvousClient::ack(
            self,
            torca_relay_protocol::RelaySlotId(slot.0),
            RelaySideToken(token.0),
            RelaySequence(up_to),
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
