use torca_communication_driver::{CommunicationError, ReadStateRuntime};
use torca_foundation::{OpaqueId, Timestamp};
use torca_storage_sqlite::SqlCipherReadState;

/// Storage-backed read-state adapter that separates local read state from the optional
/// privacy-sensitive read receipt sent to the remote peer.
pub struct PrivacyReadStateAdapter {
    read_state: SqlCipherReadState,
}

impl PrivacyReadStateAdapter {
    pub const fn new(read_state: SqlCipherReadState) -> Self {
        Self { read_state }
    }
}

impl ReadStateRuntime for PrivacyReadStateAdapter {
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.read_state
            .mark_conversation_read(conversation_id, now)
            .map(|_| ())
            .map_err(|_| CommunicationError::ReadState)
    }
}
