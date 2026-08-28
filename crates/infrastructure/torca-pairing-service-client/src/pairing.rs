use torca_pairing::PairingCode;
use torca_pairing_coordinator::{
    PairingCoordinatorError, PairingSessionDelivery, PairingSessionServicePort, PairingSideToken,
    PairingSlotCapability, PairingSlotId,
};
use torca_pairing_service_protocol::{
    PairingServiceCode, PairingServiceSequence, PairingServiceSideToken,
    PairingServiceSlotCapability,
};

use crate::RendezvousClient;

impl<T: crate::PairingServiceTransport> PairingSessionServicePort for RendezvousClient<T> {
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
        let relay_code = PairingServiceCode::new(code.as_str())
            .map_err(|_| PairingCoordinatorError::Protocol)?;
        RendezvousClient::open(
            self,
            relay_code,
            expires_at,
            creator_blob,
            PairingServiceSlotCapability(capability.0),
            PairingServiceSideToken(creator_token.0),
            ticket,
        )
        .map(|(slot, expires_at)| (PairingSlotId(slot.0), expires_at))
        .map_err(|_| PairingCoordinatorError::SessionService)
    }

    fn join(
        &mut self,
        code: &PairingCode,
        joiner_blob: Vec<u8>,
        joiner_token: PairingSideToken,
        ticket: Option<[u8; 16]>,
        _bootstrap: Option<&torca_pairing_protocol::PairingBootstrapDescriptor>,
    ) -> Result<(PairingSlotId, torca_foundation::Timestamp, Vec<u8>), PairingCoordinatorError>
    {
        let relay_code = PairingServiceCode::new(code.as_str())
            .map_err(|_| PairingCoordinatorError::Protocol)?;
        RendezvousClient::join(
            self,
            relay_code,
            joiner_blob,
            PairingServiceSideToken(joiner_token.0),
            ticket,
        )
        .map(|(slot, expires_at, creator_blob)| (PairingSlotId(slot.0), expires_at, creator_blob))
        .map_err(|_| PairingCoordinatorError::SessionService)
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
            torca_pairing_service_protocol::PairingServiceSlotId(slot.0),
            PairingServiceSideToken(token.0),
            blob,
        )
        .map_err(|_| PairingCoordinatorError::SessionService)
    }

    fn poll(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        after: u64,
    ) -> Result<Vec<PairingSessionDelivery>, PairingCoordinatorError> {
        RendezvousClient::poll(
            self,
            torca_pairing_service_protocol::PairingServiceSlotId(slot.0),
            PairingServiceSideToken(token.0),
            PairingServiceSequence(after),
        )
        .map(|deliveries| {
            deliveries
                .into_iter()
                .map(|delivery| PairingSessionDelivery {
                    sequence: delivery.sequence.0,
                    blob: delivery.blob,
                })
                .collect()
        })
        .map_err(|_| PairingCoordinatorError::SessionService)
    }

    fn ack(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        up_to: u64,
    ) -> Result<(), PairingCoordinatorError> {
        RendezvousClient::ack(
            self,
            torca_pairing_service_protocol::PairingServiceSlotId(slot.0),
            PairingServiceSideToken(token.0),
            PairingServiceSequence(up_to),
        )
        .map_err(|_| PairingCoordinatorError::SessionService)
    }

    fn close(
        &mut self,
        slot: PairingSlotId,
        capability: PairingSlotCapability,
    ) -> Result<(), PairingCoordinatorError> {
        RendezvousClient::close(
            self,
            torca_pairing_service_protocol::PairingServiceSlotId(slot.0),
            PairingServiceSlotCapability(capability.0),
        )
        .map_err(|_| PairingCoordinatorError::SessionService)
    }
}
