use crate::{CausationId, CorrelationId, EventId, Timestamp};

/// Metadata attached to an immutable domain event occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventMetadata {
    event_id: EventId,
    occurred_at: Timestamp,
    correlation_id: CorrelationId,
    causation_id: CausationId,
}

impl EventMetadata {
    /// Creates event metadata.
    pub const fn new(
        event_id: EventId,
        occurred_at: Timestamp,
        correlation_id: CorrelationId,
        causation_id: CausationId,
    ) -> Self {
        Self {
            event_id,
            occurred_at,
            correlation_id,
            causation_id,
        }
    }

    /// Returns the unique event occurrence identifier.
    pub const fn event_id(self) -> EventId {
        self.event_id
    }

    /// Returns the diagnostic occurrence timestamp.
    pub const fn occurred_at(self) -> Timestamp {
        self.occurred_at
    }

    /// Returns the logical workflow identifier.
    pub const fn correlation_id(self) -> CorrelationId {
        self.correlation_id
    }

    /// Returns the direct command or event cause.
    pub const fn causation_id(self) -> CausationId {
        self.causation_id
    }
}

/// Immutable typed domain event together with its metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainEventEnvelope<E> {
    metadata: EventMetadata,
    payload: E,
}

impl<E> DomainEventEnvelope<E> {
    /// Creates a domain event envelope.
    pub const fn new(metadata: EventMetadata, payload: E) -> Self {
        Self { metadata, payload }
    }

    /// Returns event metadata.
    pub const fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    /// Returns the domain event payload.
    pub const fn payload(&self) -> &E {
        &self.payload
    }

    /// Splits the envelope into metadata and payload.
    pub fn into_parts(self) -> (EventMetadata, E) {
        (self.metadata, self.payload)
    }

    /// Transforms the payload while preserving metadata.
    pub fn map<T>(self, mapper: impl FnOnce(E) -> T) -> DomainEventEnvelope<T> {
        DomainEventEnvelope::new(self.metadata, mapper(self.payload))
    }
}

#[cfg(test)]
mod tests {
    use crate::{CausationId, CommandId, CorrelationId, EventId, Timestamp};

    use super::{DomainEventEnvelope, EventMetadata};

    #[test]
    fn event_envelope_preserves_trace_metadata() {
        let command_id = CommandId::from_u128(1);
        let metadata = EventMetadata::new(
            EventId::from_u128(2),
            Timestamp::UNIX_EPOCH,
            CorrelationId::from(command_id),
            CausationId::from(command_id),
        );
        let envelope = DomainEventEnvelope::new(metadata, "created");

        assert_eq!(*envelope.metadata(), metadata);
        assert_eq!(*envelope.payload(), "created");
    }
}
