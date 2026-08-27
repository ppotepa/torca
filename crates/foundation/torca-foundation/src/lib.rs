//! Dependency-light shared contracts used across Torca domain and application crates.
//!
//! The crate deliberately contains no product workflow, storage, transport, cryptography,
//! serialization or platform integration. Domain crates should define their own business IDs as
//! newtypes around [`OpaqueId`] and use the shared command, event, time, cancellation and error
//! contracts only where their semantics are genuinely cross-cutting.

mod cancellation;
mod command;
mod error;
mod event;
mod id;
mod provider_id;
mod secret;
mod time;
mod wake;

pub use cancellation::{CancellationProbe, CancellationReason, Cancelled, NeverCancelled};
pub use command::{CommandEnvelope, CommandMetadata};
pub use error::{ClassifiedError, ErrorCategory, ErrorCode, ErrorDescriptor, RetryAdvice};
pub use event::{DomainEventEnvelope, EventMetadata};
pub use id::{CausationId, CommandId, CorrelationId, EventId, OpaqueId, ParseOpaqueIdError};
pub use provider_id::{InvalidProviderId, ProviderId};
pub use secret::SecretBytes;
pub use time::{Timestamp, TimestampError};
pub use wake::WakeSlot;
