use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use iroh::endpoint::{Connection, Endpoint, RecvStream};
use tokio::runtime::Runtime;
use torca_foundation::{OpaqueId, Timestamp};
use torca_pairing::PairingCode;
use torca_pairing_coordinator::{
    PairingCoordinatorError, PairingSessionDelivery, PairingSessionServicePort, PairingSideToken,
    PairingSlotCapability, PairingSlotId,
};
use torca_pairing_protocol::PairingBootstrapDescriptor;
use torca_relay_protocol::{
    RELAY_HEADER_LEN, RelayCode, RelayCodec, RelayDelivery, RelayMessageId, RelayRequest,
    RelayResponse, RelaySequence, RelaySideToken, RelaySlotCapability, RelaySlotId,
};
use torca_rendezvous_client::RendezvousClient;

use crate::{IrohIncomingRouter, IrohPairingServiceTransport};

const MAX_QUEUE: usize = 64;

struct Slot {
    code: String,
    expires_at: Timestamp,
    creator_blob: Vec<u8>,
    ticket: [u8; 16],
    capability: RelaySlotCapability,
    creator_token: RelaySideToken,
    joiner_token: Option<RelaySideToken>,
    creator_queue: VecDeque<RelayDelivery>,
    joiner_queue: VecDeque<RelayDelivery>,
    next_creator_sequence: u64,
    next_joiner_sequence: u64,
}

type Slots = Arc<Mutex<BTreeMap<RelaySlotId, Slot>>>;

/// Provider-owned direct pairing service. It uses the existing bounded slot
/// semantics, but serves them over an Iroh ALPN instead of a relay socket.
pub struct IrohPairingService {
    endpoint: Endpoint,
    runtime: Arc<Runtime>,
    slots: Slots,
    remote: BTreeMap<PairingSlotId, RendezvousClient<IrohPairingServiceTransport>>,
    local: BTreeMap<PairingSlotId, (PairingSideToken, PairingSlotCapability)>,
}

impl IrohPairingService {
    pub(crate) fn new(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
        incoming: Arc<IrohIncomingRouter>,
    ) -> Self {
        let slots = Arc::new(Mutex::new(BTreeMap::new()));
        spawn_server(incoming, Arc::clone(&runtime), Arc::clone(&slots));
        Self { endpoint, runtime, slots, remote: BTreeMap::new(), local: BTreeMap::new() }
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn pairing_bootstrap_descriptor(&self) -> Result<PairingBootstrapDescriptor, String> {
        let address = self.endpoint.addr();
        if address.is_empty() {
            return Err("Iroh endpoint has no dialable transport address yet".into());
        }
        let payload = crate::encode_endpoint_addr(&address).map_err(|error| error.to_string())?;
        PairingBootstrapDescriptor::new("iroh", payload).map_err(|error| error.to_string())
    }

    fn local_error() -> PairingCoordinatorError {
        PairingCoordinatorError::SessionService
    }

    fn local_slot(
        &self,
        slot: PairingSlotId,
        token: PairingSideToken,
    ) -> Result<RelaySlotId, PairingCoordinatorError> {
        let Some((local_token, _)) = self.local.get(&slot) else {
            return Err(Self::local_error());
        };
        if local_token.0 != token.0 {
            return Err(Self::local_error());
        }
        Ok(RelaySlotId(slot.0))
    }
}

impl PairingSessionServicePort for IrohPairingService {
    fn network_changed(&mut self) {
        for client in self.remote.values_mut() {
            client.network_changed();
        }
    }

    fn open(
        &mut self,
        code: &PairingCode,
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
        capability: PairingSlotCapability,
        creator_token: PairingSideToken,
        ticket: [u8; 16],
    ) -> Result<(PairingSlotId, Timestamp), PairingCoordinatorError> {
        let _ = RelayCode::new(code.as_str()).map_err(|_| Self::local_error())?;
        let relay_slot = RelaySlotId(random_id());
        let slot = Slot {
            code: code.as_str().to_owned(),
            expires_at,
            creator_blob,
            ticket,
            capability: RelaySlotCapability(capability.0),
            creator_token: RelaySideToken(creator_token.0),
            joiner_token: None,
            creator_queue: VecDeque::new(),
            joiner_queue: VecDeque::new(),
            next_creator_sequence: 1,
            next_joiner_sequence: 1,
        };
        self.slots.lock().map_err(|_| Self::local_error())?.insert(relay_slot, slot);
        let pairing_slot = PairingSlotId(relay_slot.0);
        self.local.insert(pairing_slot, (creator_token, capability));
        Ok((pairing_slot, expires_at))
    }

    fn join(
        &mut self,
        code: &PairingCode,
        joiner_blob: Vec<u8>,
        joiner_token: PairingSideToken,
        ticket: Option<[u8; 16]>,
        bootstrap: Option<&PairingBootstrapDescriptor>,
    ) -> Result<(PairingSlotId, Timestamp, Vec<u8>), PairingCoordinatorError> {
        let descriptor = bootstrap.ok_or(PairingCoordinatorError::BootstrapMissing)?;
        let transport = IrohPairingServiceTransport::from_bootstrap(
            self.endpoint.clone(),
            descriptor,
            Arc::clone(&self.runtime),
        )
        .map_err(|error| {
            if error == "pairing bootstrap belongs to another provider" {
                PairingCoordinatorError::BootstrapProviderMismatch
            } else {
                PairingCoordinatorError::BootstrapInvalid
            }
        })?;
        let mut client = RendezvousClient::new(transport, Duration::from_secs(15));
        let relay_code = RelayCode::new(code.as_str()).map_err(|_| Self::local_error())?;
        let (slot, expires_at, creator_blob) = client
            .join(relay_code, joiner_blob, RelaySideToken(joiner_token.0), ticket)
            .map_err(|_| Self::local_error())?;
        let pairing_slot = PairingSlotId(slot.0);
        self.remote.insert(pairing_slot, client);
        Ok((pairing_slot, expires_at, creator_blob))
    }

    fn push(
        &mut self,
        message_id: OpaqueId,
        slot: PairingSlotId,
        token: PairingSideToken,
        blob: Vec<u8>,
    ) -> Result<(), PairingCoordinatorError> {
        if let Some(client) = self.remote.get_mut(&slot) {
            return client
                .push(message_id, RelaySlotId(slot.0), RelaySideToken(token.0), blob)
                .map_err(|_| Self::local_error());
        }
        let relay_slot = self.local_slot(slot, token)?;
        let mut slots = self.slots.lock().map_err(|_| Self::local_error())?;
        let Some(entry) = slots.get_mut(&relay_slot) else {
            return Err(Self::local_error());
        };
        let is_creator = entry.creator_token.0 == token.0;
        let queue = if is_creator { &mut entry.joiner_queue } else { &mut entry.creator_queue };
        if queue.len() >= MAX_QUEUE {
            return Err(Self::local_error());
        }
        let sequence = if is_creator {
            let value = entry.next_joiner_sequence;
            entry.next_joiner_sequence = value.saturating_add(1);
            value
        } else {
            let value = entry.next_creator_sequence;
            entry.next_creator_sequence = value.saturating_add(1);
            value
        };
        queue.push_back(RelayDelivery {
            sequence: RelaySequence(sequence),
            message_id: RelayMessageId(message_id),
            blob,
        });
        Ok(())
    }

    fn poll(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        after: u64,
    ) -> Result<Vec<PairingSessionDelivery>, PairingCoordinatorError> {
        if let Some(client) = self.remote.get_mut(&slot) {
            return client
                .poll(RelaySlotId(slot.0), RelaySideToken(token.0), RelaySequence(after))
                .map(|items| {
                    items
                        .into_iter()
                        .map(|item| PairingSessionDelivery {
                            sequence: item.sequence.0,
                            blob: item.blob,
                        })
                        .collect()
                })
                .map_err(|_| Self::local_error());
        }
        let relay_slot = self.local_slot(slot, token)?;
        let slots = self.slots.lock().map_err(|_| Self::local_error())?;
        let entry = slots.get(&relay_slot).ok_or_else(Self::local_error)?;
        let queue = if entry.creator_token.0 == token.0 {
            &entry.creator_queue
        } else {
            &entry.joiner_queue
        };
        Ok(queue
            .iter()
            .filter(|item| item.sequence.0 > after)
            .map(|item| PairingSessionDelivery {
                sequence: item.sequence.0,
                blob: item.blob.clone(),
            })
            .collect())
    }

    fn ack(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        up_to: u64,
    ) -> Result<(), PairingCoordinatorError> {
        if let Some(client) = self.remote.get_mut(&slot) {
            return client
                .ack(RelaySlotId(slot.0), RelaySideToken(token.0), RelaySequence(up_to))
                .map_err(|_| Self::local_error());
        }
        let relay_slot = self.local_slot(slot, token)?;
        let mut slots = self.slots.lock().map_err(|_| Self::local_error())?;
        let entry = slots.get_mut(&relay_slot).ok_or_else(Self::local_error)?;
        let queue = if entry.creator_token.0 == token.0 {
            &mut entry.creator_queue
        } else {
            &mut entry.joiner_queue
        };
        while queue.front().is_some_and(|item| item.sequence.0 <= up_to) {
            queue.pop_front();
        }
        Ok(())
    }

    fn close(
        &mut self,
        slot: PairingSlotId,
        capability: PairingSlotCapability,
    ) -> Result<(), PairingCoordinatorError> {
        if let Some(mut client) = self.remote.remove(&slot) {
            return client
                .close(RelaySlotId(slot.0), RelaySlotCapability(capability.0))
                .map_err(|_| Self::local_error());
        }
        let relay_slot = RelaySlotId(slot.0);
        let mut slots = self.slots.lock().map_err(|_| Self::local_error())?;
        if slots.get(&relay_slot).is_some_and(|entry| entry.capability.0 == capability.0) {
            slots.remove(&relay_slot);
            self.local.remove(&slot);
            Ok(())
        } else {
            Err(Self::local_error())
        }
    }

    fn restore_creator(
        &mut self,
        slot: PairingSlotId,
        code: &PairingCode,
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
        capability: PairingSlotCapability,
        creator_token: PairingSideToken,
        ticket: [u8; 16],
    ) -> Result<(), PairingCoordinatorError> {
        let relay_code = RelayCode::new(code.as_str()).map_err(|_| Self::local_error())?;
        let relay_slot = RelaySlotId(slot.0);
        let mut slots = self.slots.lock().map_err(|_| Self::local_error())?;
        if slots.contains_key(&relay_slot) {
            return Err(PairingCoordinatorError::SessionAlreadyExists);
        }
        slots.insert(
            relay_slot,
            Slot {
                code: relay_code.as_str().to_owned(),
                expires_at,
                creator_blob,
                ticket,
                capability: RelaySlotCapability(capability.0),
                creator_token: RelaySideToken(creator_token.0),
                joiner_token: None,
                creator_queue: VecDeque::new(),
                joiner_queue: VecDeque::new(),
                next_creator_sequence: 1,
                next_joiner_sequence: 1,
            },
        );
        self.local.insert(slot, (creator_token, capability));
        Ok(())
    }
}

fn random_id() -> OpaqueId {
    let bytes = iroh::SecretKey::generate().to_bytes();
    OpaqueId::from_bytes(bytes[..16].try_into().expect("fixed identity length"))
}

fn current_timestamp() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX);
    Timestamp::from_unix_millis(millis).unwrap_or(Timestamp::UNIX_EPOCH)
}

fn spawn_server(incoming: Arc<IrohIncomingRouter>, runtime: Arc<Runtime>, slots: Slots) {
    runtime.spawn(async move {
        loop {
            let Some(connection) = incoming.take_pairing() else {
                incoming.wait_for_connection().await;
                continue;
            };
            let slots = Arc::clone(&slots);
            tokio::spawn(async move {
                serve_connection(connection, slots).await;
            });
        }
    });
}

async fn serve_connection(connection: Connection, slots: Slots) {
    // The client intentionally keeps one authenticated QUIC connection open
    // and uses a fresh bidirectional stream for every rendezvous operation.
    // Serving only the first stream makes JOIN appear to succeed, while the
    // subsequent PUSH/POLL requests wait forever and are reported as a
    // generic "endpoint not ready" error by the UI. Keep accepting streams
    // until the peer closes the connection.
    loop {
        let Ok((mut send, mut recv)) = connection.accept_bi().await else { return };
        let Ok(request) = read_request(&mut recv).await else { return };
        let response = process_request(request, &slots);
        let Ok(frame) = RelayCodec::encode_response(&response) else { return };
        if send.write_all(&frame).await.is_err() || send.finish().is_err() {
            return;
        }
    }
}

async fn read_request(recv: &mut RecvStream) -> Result<RelayRequest, ()> {
    let mut header = [0_u8; RELAY_HEADER_LEN];
    recv.read_exact(&mut header).await.map_err(|_| ())?;
    let length = RelayCodec::frame_len_from_header(&header).map_err(|_| ())?;
    let mut frame = Vec::with_capacity(length);
    frame.extend_from_slice(&header);
    let mut payload = vec![0_u8; length - RELAY_HEADER_LEN];
    recv.read_exact(&mut payload).await.map_err(|_| ())?;
    frame.extend_from_slice(&payload);
    RelayCodec::decode_request(&frame).map_err(|_| ())
}

fn process_request(request: RelayRequest, slots: &Slots) -> RelayResponse {
    match request {
        RelayRequest::Open {
            code,
            expires_at,
            creator_blob,
            slot_capability,
            creator_token,
            ticket,
            ..
        } => {
            let slot = RelaySlotId(random_id());
            let entry = Slot {
                code: code.as_str().to_owned(),
                expires_at,
                creator_blob,
                ticket: ticket.0,
                capability: slot_capability,
                creator_token,
                joiner_token: None,
                creator_queue: VecDeque::new(),
                joiner_queue: VecDeque::new(),
                next_creator_sequence: 1,
                next_joiner_sequence: 1,
            };
            if let Ok(mut map) = slots.lock() {
                map.insert(slot, entry);
            }
            RelayResponse::Opened { slot_id: slot, expires_at }
        }
        RelayRequest::Join { code, joiner_blob, joiner_token, ticket, .. } => {
            let Ok(mut map) = slots.lock() else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            let Some((slot, entry)) = map.iter_mut().find(|(_, entry)| {
                entry.code == code.as_str()
                    && entry.joiner_token.is_none()
                    && entry.creator_blob.len() <= torca_relay_protocol::MAX_RELAY_BLOB_LEN
                    && entry.expires_at >= current_timestamp()
                    && entry.ticket == ticket.map(|value| value.0).unwrap_or(entry.ticket)
            }) else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            if joiner_blob.len() > torca_relay_protocol::MAX_RELAY_BLOB_LEN
                || entry.creator_queue.len() >= MAX_QUEUE
            {
                return RelayResponse::Error(torca_relay_protocol::RelayProtocolError::QueueFull);
            }
            entry.joiner_token = Some(joiner_token);
            // The creator must receive the joiner's ephemeral public key as
            // the first delivery.  This mirrors the Tor rendezvous service;
            // dropping the blob leaves the coordinator unable to complete
            // the authenticated pairing after approval.
            let sequence = entry.next_creator_sequence;
            entry.next_creator_sequence = entry.next_creator_sequence.saturating_add(1);
            entry.creator_queue.push_back(RelayDelivery {
                sequence: RelaySequence(sequence),
                message_id: RelayMessageId(random_id()),
                blob: joiner_blob,
            });
            RelayResponse::Joined {
                slot_id: *slot,
                expires_at: entry.expires_at,
                creator_blob: entry.creator_blob.clone(),
            }
        }
        RelayRequest::Push { slot_id, token, message_id, blob, .. } => {
            let Ok(mut map) = slots.lock() else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            let Some(entry) = map.get_mut(&slot_id) else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            let is_creator = entry.creator_token.0 == token.0;
            if !is_creator && entry.joiner_token.is_none_or(|value| value.0 != token.0) {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::Unauthorized,
                );
            }
            let queue = if is_creator { &mut entry.joiner_queue } else { &mut entry.creator_queue };
            if queue.len() >= MAX_QUEUE {
                return RelayResponse::Error(torca_relay_protocol::RelayProtocolError::QueueFull);
            }
            let sequence = if is_creator {
                let value = entry.next_joiner_sequence;
                entry.next_joiner_sequence += 1;
                value
            } else {
                let value = entry.next_creator_sequence;
                entry.next_creator_sequence += 1;
                value
            };
            queue.push_back(RelayDelivery { sequence: RelaySequence(sequence), message_id, blob });
            RelayResponse::Accepted
        }
        RelayRequest::Poll { slot_id, token, after } => {
            let Ok(map) = slots.lock() else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            let Some(entry) = map.get(&slot_id) else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            let is_creator = entry.creator_token.0 == token.0;
            if !is_creator && entry.joiner_token.is_none_or(|value| value.0 != token.0) {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::Unauthorized,
                );
            }
            let queue = if is_creator { &entry.creator_queue } else { &entry.joiner_queue };
            RelayResponse::Deliveries(
                queue.iter().filter(|item| item.sequence.0 > after.0).cloned().collect(),
            )
        }
        RelayRequest::Ack { slot_id, token, up_to } => {
            let Ok(mut map) = slots.lock() else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            let Some(entry) = map.get_mut(&slot_id) else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            let is_creator = entry.creator_token.0 == token.0;
            if !is_creator && entry.joiner_token.is_none_or(|value| value.0 != token.0) {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::Unauthorized,
                );
            }
            let queue = if is_creator { &mut entry.creator_queue } else { &mut entry.joiner_queue };
            while queue.front().is_some_and(|item| item.sequence.0 <= up_to.0) {
                queue.pop_front();
            }
            RelayResponse::Acked(up_to)
        }
        RelayRequest::Close { slot_id, capability } => {
            let Ok(mut map) = slots.lock() else {
                return RelayResponse::Error(
                    torca_relay_protocol::RelayProtocolError::SlotNotFound,
                );
            };
            if map.get(&slot_id).is_some_and(|entry| entry.capability.0 == capability.0) {
                map.remove(&slot_id);
                RelayResponse::Closed
            } else {
                RelayResponse::Error(torca_relay_protocol::RelayProtocolError::Unauthorized)
            }
        }
        RelayRequest::Health => RelayResponse::Healthy,
        RelayRequest::Info => RelayResponse::Info(
            torca_relay_protocol::RelayInfo::new("torca-iroh", "direct", "direct")
                .expect("static info"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_slot_server_preserves_bidirectional_queue_semantics() {
        let slots = Arc::new(Mutex::new(BTreeMap::new()));
        let code = RelayCode::new("ABC123").expect("code");
        let slot_capability = RelaySlotCapability(OpaqueId::from_bytes([3; 16]));
        let creator_token = RelaySideToken(OpaqueId::from_bytes([1; 16]));
        let joiner_token = RelaySideToken(OpaqueId::from_bytes([2; 16]));
        let opened = process_request(
            RelayRequest::Open {
                operation_id: torca_relay_protocol::RelayOperationId(OpaqueId::from_bytes([9; 16])),
                code: code.clone(),
                expires_at: Timestamp::from_unix_millis(
                    current_timestamp().to_unix_millis().saturating_add(60_000),
                )
                .expect("timestamp"),
                creator_blob: b"creator".to_vec(),
                slot_capability,
                creator_token,
                ticket: torca_relay_protocol::RelayJoinTicket([4; 16]),
            },
            &slots,
        );
        let RelayResponse::Opened { slot_id, .. } = opened else { panic!("open response") };
        let joined = process_request(
            RelayRequest::Join {
                operation_id: torca_relay_protocol::RelayOperationId(OpaqueId::from_bytes([8; 16])),
                code,
                joiner_blob: b"joiner".to_vec(),
                joiner_token,
                ticket: Some(torca_relay_protocol::RelayJoinTicket([4; 16])),
            },
            &slots,
        );
        assert!(matches!(joined, RelayResponse::Joined { slot_id: id, .. } if id == slot_id));
        let pushed = process_request(
            RelayRequest::Push {
                operation_id: torca_relay_protocol::RelayOperationId(OpaqueId::from_bytes([7; 16])),
                message_id: RelayMessageId(OpaqueId::from_bytes([6; 16])),
                slot_id,
                token: joiner_token,
                blob: b"hello".to_vec(),
            },
            &slots,
        );
        assert_eq!(pushed, RelayResponse::Accepted);
        let polled = process_request(
            RelayRequest::Poll { slot_id, token: creator_token, after: RelaySequence(0) },
            &slots,
        );
        assert!(matches!(polled, RelayResponse::Deliveries(ref items) if items.len() == 2
                && items[0].blob == b"joiner"
                && items[1].blob == b"hello"));
    }
}
