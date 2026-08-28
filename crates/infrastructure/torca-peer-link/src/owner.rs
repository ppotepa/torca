//! Authenticated Tor peer-link owner for the Torca runtime.

mod ack;
mod reconnect;

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ack::{AckWaitAction, classify_ack_wait_message, link_ack};
use reconnect::{ReconnectEntry, ReconnectReason, reconnect_delay};
use torca_connectivity::{
    ConnectivityObserver, OperationPhase, TransportDirection, TransportLayer, TransportOperation,
    TransportStage,
};
use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, ContactStatus, PeerCredentialRepository,
};
use torca_crypto::{CryptoProvider, Ed25519HandshakeVerifier, RustCryptoProvider};
use torca_foundation::{OpaqueId, ProviderId, Timestamp};
use torca_peer::{PeerSession, PeerSessionError, PeerSessionState, PeerTransport};
use torca_peer_protocol::{
    AckStatus, HandshakeAck, HandshakeHello, HandshakePolicy, HandshakeSigner, PeerCodec,
    PeerMessage,
};
use torca_transport_api::TransportCapabilities;

pub use torca_transport_api::PeerTransportFactory;

const MAX_CLOCK_SKEW_MS: i64 = 2 * 60 * 1000;
const MAX_PENDING_INCOMING: usize = 64;
const MAX_INBOUND_EVENTS: usize = 256;
const MAX_PENDING_ACKS: usize = 256;
const MAX_ACK_WAIT_SLICE: Duration = Duration::from_secs(1);
const PEER_RECOVERY_WINDOW: Duration = Duration::from_secs(30);
const PEER_RECOVERY_TICK: Duration = Duration::from_millis(250);

include!("owner/model.rs");

include!("owner/handshake_methods.rs");
include!("owner/telemetry_methods.rs");
include!("owner/session_methods.rs");

include!("owner/support.rs");
include!("owner/public_methods.rs");
