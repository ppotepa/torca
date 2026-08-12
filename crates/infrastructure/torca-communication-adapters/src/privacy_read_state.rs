use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use torca_communication_driver::{CommunicationError, ReadStateRuntime, plan_read_receipts};
use torca_foundation::{OpaqueId, Timestamp};
use torca_storage_sqlite::SqlCipherReadState;

/// Process-wide privacy policy shared by the native settings boundary and the
/// communication worker. Relaxed ordering is sufficient because this flag does
/// not guard any other state.
#[derive(Clone, Debug)]
pub struct ReadReceiptPolicy(Arc<AtomicBool>);

impl ReadReceiptPolicy {
    pub fn new(enabled: bool) -> Self {
        Self(Arc::new(AtomicBool::new(enabled)))
    }

    pub fn is_enabled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.0.store(enabled, Ordering::Relaxed);
    }
}

/// Storage-backed read-state adapter that separates local read state from the optional
/// privacy-sensitive read receipt sent to the remote peer.
pub struct PrivacyReadStateAdapter {
    read_state: SqlCipherReadState,
    policy: ReadReceiptPolicy,
}

impl PrivacyReadStateAdapter {
    pub const fn new(read_state: SqlCipherReadState, policy: ReadReceiptPolicy) -> Self {
        Self { read_state, policy }
    }
}

impl ReadStateRuntime for PrivacyReadStateAdapter {
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        if !self.policy.is_enabled() {
            return self
                .read_state
                .mark_conversation_read(conversation_id, now)
                .map(|_| ())
                .map_err(|_| CommunicationError::ReadState);
        }
        let candidates = self
            .read_state
            .read_candidates(conversation_id)
            .map_err(|_| CommunicationError::ReadState)?;
        let jobs = plan_read_receipts(&candidates, now)?;
        self.read_state
            .commit_mark_read(conversation_id, now, &jobs)
            .map(|_| ())
            .map_err(|_| CommunicationError::ReadState)
    }
}

#[cfg(test)]
mod tests {
    use super::ReadReceiptPolicy;

    #[test]
    fn policy_updates_are_visible_to_existing_clones() {
        let policy = ReadReceiptPolicy::new(true);
        let worker_policy = policy.clone();
        policy.set_enabled(false);
        assert!(!worker_policy.is_enabled());
    }
}
