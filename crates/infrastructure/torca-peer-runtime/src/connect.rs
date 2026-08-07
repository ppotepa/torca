use torca_contacts::{ContactId, ContactRepository, PeerCredentialRepository};
use torca_foundation::Timestamp;
use torca_peer::PeerSessionState;
use torca_peer_protocol::HandshakeSigner;

use crate::core::{PeerRuntime, PeerRuntimeError};

impl<S, K> PeerRuntime<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    /// Ensures one usable connection attempt exists. Returns true only when a new outgoing
    /// connection was actually started.
    pub fn ensure_connected(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<bool, PeerRuntimeError> {
        if let Some(session) = self.incoming.get(&contact_id) {
            if matches!(
                session.state(),
                PeerSessionState::Connecting
                    | PeerSessionState::Handshaking
                    | PeerSessionState::Ready
            ) {
                return Ok(false);
            }
        }
        if let Some(session) = self.outgoing.get(&contact_id) {
            if matches!(
                session.state(),
                PeerSessionState::Connecting
                    | PeerSessionState::Handshaking
                    | PeerSessionState::Ready
                    | PeerSessionState::Reconnecting
            ) {
                return Ok(false);
            }
        }

        self.incoming.remove(&contact_id);
        self.outgoing.remove(&contact_id);
        self.connect_contact(contact_id, now)?;
        Ok(true)
    }
}
