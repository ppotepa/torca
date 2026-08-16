//! Authenticated Tor peer-link owner for the Torca runtime.

mod ack;
mod reconnect;

use core::fmt;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ack::{AckWaitAction, classify_ack_wait_message, link_ack};
use reconnect::{ReconnectEntry, reconnect_delay};
use torca_connectivity::{
    ConnectivityObserver, OperationPhase, TransportDirection, TransportLayer, TransportOperation,
};
use torca_contacts::{
    Contact, ContactError, ContactId, ContactRepository, PeerCredentialRepository,
};
use torca_crypto::{CryptoProvider, Ed25519HandshakeVerifier, RustCryptoProvider};
use torca_foundation::{OpaqueId, Timestamp};
use torca_peer::{PeerSession, PeerSessionError, PeerSessionState, PeerTransport};
use torca_peer_protocol::{
    AckStatus, HandshakeAck, HandshakeHello, HandshakePolicy, HandshakeSigner, PeerCodec,
    PeerMessage,
};
use torca_tor::{
    PeerListener, TOR_PEER_VIRTUAL_PORT, TorPeerTransport, TorServiceHandle, TransportError,
};

const MAX_CLOCK_SKEW_MS: i64 = 2 * 60 * 1000;
const MAX_PENDING_INCOMING: usize = 64;
const MAX_INBOUND_EVENTS: usize = 256;
const MAX_PENDING_ACKS: usize = 256;
const MAX_ACK_WAIT_SLICE: Duration = Duration::from_secs(1);

include!("owner/model.rs");

impl<S, K> PeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    include!("owner/public_methods.rs");
    include!("owner/handshake_methods.rs");
    include!("owner/telemetry_methods.rs");
    include!("owner/session_methods.rs");
}

include!("owner/support.rs");
