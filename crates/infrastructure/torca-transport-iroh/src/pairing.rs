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
use torca_pairing_service_client::RendezvousClient;
use torca_pairing_service_protocol::{
    PAIRING_SERVICE_HEADER_LEN, PairingServiceCode, PairingServiceCodec, PairingServiceDelivery,
    PairingServiceMessageId, PairingServiceRequest, PairingServiceResponse, PairingServiceSequence,
    PairingServiceSideToken, PairingServiceSlotCapability, PairingServiceSlotId,
};

use crate::{
    IrohEndpointSlot, IrohIncomingRouter, IrohPairingServiceTransport, ProviderEndpointSlot,
};

const MAX_QUEUE: usize = 64;

struct Slot {
    code: String,
    expires_at: Timestamp,
    creator_blob: Vec<u8>,
    ticket: [u8; 16],
    capability: PairingServiceSlotCapability,
    creator_token: PairingServiceSideToken,
    joiner_token: Option<PairingServiceSideToken>,
    creator_queue: VecDeque<PairingServiceDelivery>,
    joiner_queue: VecDeque<PairingServiceDelivery>,
    next_creator_sequence: u64,
    next_joiner_sequence: u64,
}

type Slots = Arc<Mutex<BTreeMap<PairingServiceSlotId, Slot>>>;

/// Provider-owned direct pairing service. It uses the existing bounded slot
/// semantics, but serves them over an Iroh ALPN instead of a relay socket.
pub struct IrohPairingService {
    endpoint: ProviderEndpointSlot,
    runtime: Arc<Runtime>,
    slots: Slots,
    remote: BTreeMap<PairingSlotId, RendezvousClient<IrohPairingServiceTransport>>,
    local: BTreeMap<PairingSlotId, (PairingSideToken, PairingSlotCapability)>,
}

impl IrohPairingService {
    #[allow(dead_code)]
    pub(crate) fn new(
        endpoint: Endpoint,
        runtime: Arc<Runtime>,
        incoming: Arc<IrohIncomingRouter>,
    ) -> Self {
        Self::new_with_slot(
            IrohEndpointSlot::static_endpoint(endpoint, Arc::clone(&runtime)),
            runtime,
            incoming,
        )
    }

    pub(crate) fn new_with_slot(
        endpoint: ProviderEndpointSlot,
        runtime: Arc<Runtime>,
        incoming: Arc<IrohIncomingRouter>,
    ) -> Self {
        let slots = Arc::new(Mutex::new(BTreeMap::new()));
        spawn_server(incoming, Arc::clone(&runtime), Arc::clone(&slots));
        Self { endpoint, runtime, slots, remote: BTreeMap::new(), local: BTreeMap::new() }
    }

    pub fn endpoint(&self) -> Option<Endpoint> {
        self.endpoint.current()
    }

    pub fn endpoint_slot(&self) -> ProviderEndpointSlot {
        Arc::clone(&self.endpoint)
    }

    pub fn pairing_bootstrap_descriptor(&self) -> Result<PairingBootstrapDescriptor, String> {
        if !self.endpoint.route_is_fresh() {
            return Err("Iroh endpoint route is migrating".to_owned());
        }
        let address =
            self.endpoint.address().ok_or_else(|| "Iroh endpoint is dormant".to_owned())?;
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
    ) -> Result<PairingServiceSlotId, PairingCoordinatorError> {
        let Some((local_token, _)) = self.local.get(&slot) else {
            return Err(Self::local_error());
        };
        if local_token.0 != token.0 {
            return Err(Self::local_error());
        }
        Ok(PairingServiceSlotId(slot.0))
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
        purge_expired_slots(&self.slots);
        let _ = PairingServiceCode::new(code.as_str()).map_err(|_| Self::local_error())?;
        let relay_slot = PairingServiceSlotId(random_id());
        let slot = Slot {
            code: code.as_str().to_owned(),
            expires_at,
            creator_blob,
            ticket,
            capability: PairingServiceSlotCapability(capability.0),
            creator_token: PairingServiceSideToken(creator_token.0),
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
            Arc::clone(&self.endpoint),
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
        let relay_code = PairingServiceCode::new(code.as_str()).map_err(|_| Self::local_error())?;
        let (slot, expires_at, creator_blob) = client
            .join(relay_code, joiner_blob, PairingServiceSideToken(joiner_token.0), ticket)
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
        purge_expired_slots(&self.slots);
        if let Some(client) = self.remote.get_mut(&slot) {
            return client
                .push(
                    message_id,
                    PairingServiceSlotId(slot.0),
                    PairingServiceSideToken(token.0),
                    blob,
                )
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
        queue.push_back(PairingServiceDelivery {
            sequence: PairingServiceSequence(sequence),
            message_id: PairingServiceMessageId(message_id),
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
        purge_expired_slots(&self.slots);
        if let Some(client) = self.remote.get_mut(&slot) {
            return client
                .poll(
                    PairingServiceSlotId(slot.0),
                    PairingServiceSideToken(token.0),
                    PairingServiceSequence(after),
                )
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
        purge_expired_slots(&self.slots);
        if let Some(client) = self.remote.get_mut(&slot) {
            return client
                .ack(
                    PairingServiceSlotId(slot.0),
                    PairingServiceSideToken(token.0),
                    PairingServiceSequence(up_to),
                )
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
        purge_expired_slots(&self.slots);
        if let Some(mut client) = self.remote.remove(&slot) {
            return client
                .close(PairingServiceSlotId(slot.0), PairingServiceSlotCapability(capability.0))
                .map_err(|_| Self::local_error());
        }
        let relay_slot = PairingServiceSlotId(slot.0);
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
        purge_expired_slots(&self.slots);
        let relay_code = PairingServiceCode::new(code.as_str()).map_err(|_| Self::local_error())?;
        let relay_slot = PairingServiceSlotId(slot.0);
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
                capability: PairingServiceSlotCapability(capability.0),
                creator_token: PairingServiceSideToken(creator_token.0),
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
                if !incoming.wait_for_connection().await {
                    break;
                }
                continue;
            };
            let slots = Arc::clone(&slots);
            tokio::spawn(async move {
                serve_connection(connection, slots).await;
            });
        }
    });
}

fn purge_expired_slots(slots: &Slots) {
    let Ok(mut map) = slots.lock() else { return };
    let now = current_timestamp();
    map.retain(|_, entry| entry.expires_at > now);
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
        let Ok(frame) = PairingServiceCodec::encode_response(&response) else { return };
        if send.write_all(&frame).await.is_err() || send.finish().is_err() {
            return;
        }
    }
}

async fn read_request(recv: &mut RecvStream) -> Result<PairingServiceRequest, ()> {
    let mut header = [0_u8; PAIRING_SERVICE_HEADER_LEN];
    recv.read_exact(&mut header).await.map_err(|_| ())?;
    let length = PairingServiceCodec::frame_len_from_header(&header).map_err(|_| ())?;
    let mut frame = Vec::with_capacity(length);
    frame.extend_from_slice(&header);
    let mut payload = vec![0_u8; length - PAIRING_SERVICE_HEADER_LEN];
    recv.read_exact(&mut payload).await.map_err(|_| ())?;
    frame.extend_from_slice(&payload);
    PairingServiceCodec::decode_request(&frame).map_err(|_| ())
}

fn process_request(request: PairingServiceRequest, slots: &Slots) -> PairingServiceResponse {
    // Pairing slots are intentionally in-memory and short-lived. Purging on
    // every request keeps expired invitations from accumulating when a peer
    // never sends the final Close, and ensures expired tokens cannot continue
    // to push/poll after the five-minute invitation window.
    purge_expired_slots(slots);
    match request {
        PairingServiceRequest::Open {
            code,
            expires_at,
            creator_blob,
            slot_capability,
            creator_token,
            ticket,
            ..
        } => {
            let slot = PairingServiceSlotId(random_id());
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
            PairingServiceResponse::Opened { slot_id: slot, expires_at }
        }
        PairingServiceRequest::Join { code, joiner_blob, joiner_token, ticket, .. } => {
            let Ok(mut map) = slots.lock() else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            let Some((slot, entry)) = map.iter_mut().find(|(_, entry)| {
                entry.code == code.as_str()
                    && entry.joiner_token.is_none()
                    && entry.creator_blob.len()
                        <= torca_pairing_service_protocol::MAX_PAIRING_SERVICE_BLOB_LEN
                    && entry.expires_at >= current_timestamp()
                    && entry.ticket == ticket.map(|value| value.0).unwrap_or(entry.ticket)
            }) else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            if joiner_blob.len() > torca_pairing_service_protocol::MAX_PAIRING_SERVICE_BLOB_LEN
                || entry.creator_queue.len() >= MAX_QUEUE
            {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::QueueFull,
                );
            }
            entry.joiner_token = Some(joiner_token);
            // The creator must receive the joiner's ephemeral public key as
            // the first delivery.  This mirrors the Tor rendezvous service;
            // dropping the blob leaves the coordinator unable to complete
            // the authenticated pairing after approval.
            let sequence = entry.next_creator_sequence;
            entry.next_creator_sequence = entry.next_creator_sequence.saturating_add(1);
            entry.creator_queue.push_back(PairingServiceDelivery {
                sequence: PairingServiceSequence(sequence),
                message_id: PairingServiceMessageId(random_id()),
                blob: joiner_blob,
            });
            PairingServiceResponse::Joined {
                slot_id: *slot,
                expires_at: entry.expires_at,
                creator_blob: entry.creator_blob.clone(),
            }
        }
        PairingServiceRequest::Push { slot_id, token, message_id, blob, .. } => {
            let Ok(mut map) = slots.lock() else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            let Some(entry) = map.get_mut(&slot_id) else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            let is_creator = entry.creator_token.0 == token.0;
            if !is_creator && entry.joiner_token.is_none_or(|value| value.0 != token.0) {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::Unauthorized,
                );
            }
            let queue = if is_creator { &mut entry.joiner_queue } else { &mut entry.creator_queue };
            if queue.len() >= MAX_QUEUE {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::QueueFull,
                );
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
            queue.push_back(PairingServiceDelivery {
                sequence: PairingServiceSequence(sequence),
                message_id,
                blob,
            });
            PairingServiceResponse::Accepted
        }
        PairingServiceRequest::Poll { slot_id, token, after } => {
            let Ok(map) = slots.lock() else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            let Some(entry) = map.get(&slot_id) else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            let is_creator = entry.creator_token.0 == token.0;
            if !is_creator && entry.joiner_token.is_none_or(|value| value.0 != token.0) {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::Unauthorized,
                );
            }
            let queue = if is_creator { &entry.creator_queue } else { &entry.joiner_queue };
            PairingServiceResponse::Deliveries(
                queue.iter().filter(|item| item.sequence.0 > after.0).cloned().collect(),
            )
        }
        PairingServiceRequest::Ack { slot_id, token, up_to } => {
            let Ok(mut map) = slots.lock() else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            let Some(entry) = map.get_mut(&slot_id) else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            let is_creator = entry.creator_token.0 == token.0;
            if !is_creator && entry.joiner_token.is_none_or(|value| value.0 != token.0) {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::Unauthorized,
                );
            }
            let queue = if is_creator { &mut entry.creator_queue } else { &mut entry.joiner_queue };
            while queue.front().is_some_and(|item| item.sequence.0 <= up_to.0) {
                queue.pop_front();
            }
            PairingServiceResponse::Acked(up_to)
        }
        PairingServiceRequest::Close { slot_id, capability } => {
            let Ok(mut map) = slots.lock() else {
                return PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound,
                );
            };
            if map.get(&slot_id).is_some_and(|entry| entry.capability.0 == capability.0) {
                map.remove(&slot_id);
                PairingServiceResponse::Closed
            } else {
                PairingServiceResponse::Error(
                    torca_pairing_service_protocol::PairingServiceProtocolError::Unauthorized,
                )
            }
        }
        PairingServiceRequest::Health => PairingServiceResponse::Healthy,
        PairingServiceRequest::Info => PairingServiceResponse::Info(
            torca_pairing_service_protocol::PairingServiceInfo::new(
                "torca-iroh",
                "direct",
                "direct",
            )
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
        let code = PairingServiceCode::new("ABC123").expect("code");
        let slot_capability = PairingServiceSlotCapability(OpaqueId::from_bytes([3; 16]));
        let creator_token = PairingServiceSideToken(OpaqueId::from_bytes([1; 16]));
        let joiner_token = PairingServiceSideToken(OpaqueId::from_bytes([2; 16]));
        let opened = process_request(
            PairingServiceRequest::Open {
                operation_id: torca_pairing_service_protocol::PairingServiceOperationId(
                    OpaqueId::from_bytes([9; 16]),
                ),
                code: code.clone(),
                expires_at: Timestamp::from_unix_millis(
                    current_timestamp().to_unix_millis().saturating_add(60_000),
                )
                .expect("timestamp"),
                creator_blob: b"creator".to_vec(),
                slot_capability,
                creator_token,
                ticket: torca_pairing_service_protocol::PairingServiceJoinTicket([4; 16]),
            },
            &slots,
        );
        let PairingServiceResponse::Opened { slot_id, .. } = opened else {
            panic!("open response")
        };
        let joined = process_request(
            PairingServiceRequest::Join {
                operation_id: torca_pairing_service_protocol::PairingServiceOperationId(
                    OpaqueId::from_bytes([8; 16]),
                ),
                code,
                joiner_blob: b"joiner".to_vec(),
                joiner_token,
                ticket: Some(torca_pairing_service_protocol::PairingServiceJoinTicket([4; 16])),
            },
            &slots,
        );
        assert!(
            matches!(joined, PairingServiceResponse::Joined { slot_id: id, .. } if id == slot_id)
        );
        let pushed = process_request(
            PairingServiceRequest::Push {
                operation_id: torca_pairing_service_protocol::PairingServiceOperationId(
                    OpaqueId::from_bytes([7; 16]),
                ),
                message_id: PairingServiceMessageId(OpaqueId::from_bytes([6; 16])),
                slot_id,
                token: joiner_token,
                blob: b"hello".to_vec(),
            },
            &slots,
        );
        assert_eq!(pushed, PairingServiceResponse::Accepted);
        let polled = process_request(
            PairingServiceRequest::Poll {
                slot_id,
                token: creator_token,
                after: PairingServiceSequence(0),
            },
            &slots,
        );
        assert!(matches!(polled, PairingServiceResponse::Deliveries(ref items) if items.len() == 2
                && items[0].blob == b"joiner"
                && items[1].blob == b"hello"));
    }

    #[test]
    fn expired_slots_are_purged_before_requests_are_served() {
        let slots = Arc::new(Mutex::new(BTreeMap::new()));
        let slot_id = PairingServiceSlotId(OpaqueId::from_bytes([7; 16]));
        let token = PairingServiceSideToken(OpaqueId::from_bytes([8; 16]));
        slots.lock().expect("slots").insert(
            slot_id,
            Slot {
                code: "EXPIRED".to_owned(),
                expires_at: Timestamp::from_unix_millis(
                    current_timestamp().to_unix_millis().saturating_sub(1),
                )
                .expect("timestamp"),
                creator_blob: Vec::new(),
                ticket: [0; 16],
                capability: PairingServiceSlotCapability(OpaqueId::from_bytes([9; 16])),
                creator_token: token,
                joiner_token: None,
                creator_queue: VecDeque::new(),
                joiner_queue: VecDeque::new(),
                next_creator_sequence: 1,
                next_joiner_sequence: 1,
            },
        );

        let response = process_request(
            PairingServiceRequest::Poll { slot_id, token, after: PairingServiceSequence(0) },
            &slots,
        );
        assert!(matches!(
            response,
            PairingServiceResponse::Error(
                torca_pairing_service_protocol::PairingServiceProtocolError::SlotNotFound
            )
        ));
        assert!(slots.lock().expect("slots").is_empty());
    }
}
