use std::collections::BTreeMap;

use torca_foundation::OpaqueId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PendingOperationKind {
    CreatePairing,
    JoinPairing { code: String, ticket: Option<[u8; 16]> },
    ApprovePairing,
    RejectPairing,
    CancelPairing,
    RenameContact { display_name: String },
    VerifyContact,
    ResetContactVerification,
    BlockContact,
    UnblockContact,
    RemoveContact,
    ClearConversationHistory,
    MarkConversationRead,
}

impl PendingOperationKind {
    pub const fn discriminator(&self) -> u8 {
        match self {
            Self::CreatePairing => 1,
            Self::JoinPairing { .. } => 2,
            Self::ApprovePairing => 3,
            Self::RejectPairing => 4,
            Self::CancelPairing => 5,
            Self::RenameContact { .. } => 16,
            Self::VerifyContact => 17,
            Self::ResetContactVerification => 18,
            Self::BlockContact => 19,
            Self::UnblockContact => 20,
            Self::RemoveContact => 21,
            Self::ClearConversationHistory => 32,
            Self::MarkConversationRead => 33,
        }
    }
}

pub fn pending_operation_id(resource_id: OpaqueId, kind: &PendingOperationKind) -> OpaqueId {
    OpaqueId::from_u128(resource_id.to_u128() ^ u128::from(kind.discriminator()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingOperation {
    pub id: OpaqueId,
    pub resource_id: OpaqueId,
    pub kind: PendingOperationKind,
    pub attempts: u32,
    pub next_attempt_at_ms: i64,
    pub created_at_ms: i64,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingOperationStoreError {
    Unavailable,
}

pub trait PendingOperationStore: Send {
    fn enqueue(&mut self, operation: PendingOperation) -> Result<(), PendingOperationStoreError>;
    fn due(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> Result<Vec<PendingOperation>, PendingOperationStoreError>;
    fn complete(&mut self, id: OpaqueId) -> Result<(), PendingOperationStoreError>;
    fn reschedule(
        &mut self,
        id: OpaqueId,
        attempts: u32,
        next_attempt_at_ms: i64,
        error: &str,
    ) -> Result<(), PendingOperationStoreError>;

    /// Returns the earliest durable retry deadline. `None` means the store is
    /// empty or the backend cannot expose a deadline; callers must then wait
    /// for an explicit wake instead of inventing a polling interval.
    fn next_due_at_ms(&self) -> Result<Option<i64>, PendingOperationStoreError> {
        Ok(None)
    }

    fn all(&self) -> Result<Vec<PendingOperation>, PendingOperationStoreError> {
        // Keep the projection bounded and representable by every persistence
        // backend. `usize::MAX` cannot be passed to SQLite's signed LIMIT on
        // 64-bit targets and made the root snapshot fail during startup.
        self.due(i64::MAX, 1_024)
    }
}

#[derive(Default)]
pub struct InMemoryPendingOperationStore {
    operations: BTreeMap<OpaqueId, PendingOperation>,
}

impl PendingOperationStore for InMemoryPendingOperationStore {
    fn enqueue(&mut self, operation: PendingOperation) -> Result<(), PendingOperationStoreError> {
        self.operations.entry(operation.id).or_insert(operation);
        Ok(())
    }

    fn due(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> Result<Vec<PendingOperation>, PendingOperationStoreError> {
        Ok(self
            .operations
            .values()
            .filter(|operation| operation.next_attempt_at_ms <= now_ms)
            .take(limit)
            .cloned()
            .collect())
    }

    fn next_due_at_ms(&self) -> Result<Option<i64>, PendingOperationStoreError> {
        Ok(self.operations.values().map(|operation| operation.next_attempt_at_ms).min())
    }

    fn complete(&mut self, id: OpaqueId) -> Result<(), PendingOperationStoreError> {
        self.operations.remove(&id);
        Ok(())
    }

    fn reschedule(
        &mut self,
        id: OpaqueId,
        attempts: u32,
        next_attempt_at_ms: i64,
        error: &str,
    ) -> Result<(), PendingOperationStoreError> {
        let operation =
            self.operations.get_mut(&id).ok_or(PendingOperationStoreError::Unavailable)?;
        operation.attempts = attempts;
        operation.next_attempt_at_ms = next_attempt_at_ms;
        operation.last_error = Some(error.into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(id: u128, due: i64) -> PendingOperation {
        let resource_id = OpaqueId::from_u128(id);
        let kind = PendingOperationKind::ApprovePairing;
        PendingOperation {
            id: pending_operation_id(resource_id, &kind),
            resource_id,
            kind,
            attempts: 0,
            next_attempt_at_ms: due,
            created_at_ms: 0,
            last_error: None,
        }
    }

    #[test]
    fn next_due_is_empty_without_pending_work() {
        let store = InMemoryPendingOperationStore::default();
        assert_eq!(store.next_due_at_ms().expect("deadline"), None);
    }

    #[test]
    fn next_due_selects_earliest_retry() {
        let mut store = InMemoryPendingOperationStore::default();
        store.enqueue(operation(1, 500)).expect("enqueue");
        store.enqueue(operation(2, 100)).expect("enqueue");
        assert_eq!(store.next_due_at_ms().expect("deadline"), Some(100));
    }
}
