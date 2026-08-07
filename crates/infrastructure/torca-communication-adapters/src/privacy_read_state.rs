use torca_communication_driver::{CommunicationError, ReadStateRuntime};
use torca_foundation::{OpaqueId, Timestamp};
use torca_read_state::SqlCipherReadState;

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
        self.mark_conversation_read_with_policy(conversation_id, now, true)
    }

    fn mark_conversation_read_with_policy(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
        send_receipt: bool,
    ) -> Result<(), CommunicationError> {
        self.read_state
            .mark_conversation_read_with_policy(conversation_id, now, send_receipt)
            .map(|_| ())
            .map_err(|_| CommunicationError::ReadState)
    }
}
