use crate::{CausationId, CommandId, CorrelationId, Timestamp};

/// Metadata required for idempotent command processing and diagnostic tracing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandMetadata {
    command_id: CommandId,
    issued_at: Timestamp,
    correlation_id: CorrelationId,
    causation_id: Option<CausationId>,
}

impl CommandMetadata {
    /// Creates metadata for a root command and derives its correlation identifier from the command identifier.
    pub const fn root(command_id: CommandId, issued_at: Timestamp) -> Self {
        Self {
            command_id,
            issued_at,
            correlation_id: CorrelationId::from_opaque(command_id.as_opaque()),
            causation_id: None,
        }
    }

    /// Creates metadata for a command participating in an existing workflow.
    pub const fn correlated(
        command_id: CommandId,
        issued_at: Timestamp,
        correlation_id: CorrelationId,
        causation_id: CausationId,
    ) -> Self {
        Self {
            command_id,
            issued_at,
            correlation_id,
            causation_id: Some(causation_id),
        }
    }

    /// Returns the stable idempotency identifier.
    pub const fn command_id(self) -> CommandId {
        self.command_id
    }

    /// Returns the diagnostic issue timestamp.
    pub const fn issued_at(self) -> Timestamp {
        self.issued_at
    }

    /// Returns the logical workflow identifier.
    pub const fn correlation_id(self) -> CorrelationId {
        self.correlation_id
    }

    /// Returns the direct cause when this is not a root command.
    pub const fn causation_id(self) -> Option<CausationId> {
        self.causation_id
    }
}

/// Typed application command together with its processing metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandEnvelope<C> {
    metadata: CommandMetadata,
    payload: C,
}

impl<C> CommandEnvelope<C> {
    /// Creates a command envelope.
    pub const fn new(metadata: CommandMetadata, payload: C) -> Self {
        Self { metadata, payload }
    }

    /// Returns command metadata.
    pub const fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    /// Returns the command payload.
    pub const fn payload(&self) -> &C {
        &self.payload
    }

    /// Splits the envelope into metadata and payload.
    pub fn into_parts(self) -> (CommandMetadata, C) {
        (self.metadata, self.payload)
    }

    /// Transforms the payload while preserving metadata.
    pub fn map<T>(self, mapper: impl FnOnce(C) -> T) -> CommandEnvelope<T> {
        CommandEnvelope::new(self.metadata, mapper(self.payload))
    }
}

#[cfg(test)]
mod tests {
    use crate::{CommandId, CorrelationId, OpaqueId, Timestamp};

    use super::{CommandEnvelope, CommandMetadata};

    #[test]
    fn root_command_uses_its_command_identifier_as_correlation_identifier() {
        let command_id = CommandId::from_u128(7);
        let metadata = CommandMetadata::root(command_id, Timestamp::UNIX_EPOCH);

        assert_eq!(metadata.command_id(), command_id);
        assert_eq!(metadata.correlation_id(), CorrelationId::from(command_id));
        assert_eq!(metadata.causation_id(), None);
    }

    #[test]
    fn mapping_a_command_preserves_metadata() {
        let metadata = CommandMetadata::root(
            CommandId::from_opaque(OpaqueId::from_u128(9)),
            Timestamp::UNIX_EPOCH,
        );
        let envelope = CommandEnvelope::new(metadata, 4_u8).map(u16::from);

        assert_eq!(*envelope.metadata(), metadata);
        assert_eq!(*envelope.payload(), 4_u16);
    }
}
