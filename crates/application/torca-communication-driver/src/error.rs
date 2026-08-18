use core::fmt;

use torca_foundation::{ClassifiedError, ErrorCategory, ErrorCode, ErrorDescriptor, RetryAdvice};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommunicationError {
    Peer,
    Text,
    Control,
    Inbound,
    Attachment,
    AttachmentStage(AttachmentFailureStage),
    ReadState,
    Relationship,
    Engine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentFailureStage {
    AckTimeout,
    PeerUnavailable,
    Integrity,
    Storage,
    Dependency,
    Protocol,
    Unknown,
}

impl fmt::Display for CommunicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CommunicationError {}

impl ClassifiedError for CommunicationError {
    fn descriptor(&self) -> ErrorDescriptor {
        let (code, category, retry) = match self {
            Self::Peer => {
                ("communication.peer_unavailable", ErrorCategory::Unavailable, RetryAdvice::Backoff)
            }
            Self::Text => {
                ("communication.text_failed", ErrorCategory::Unavailable, RetryAdvice::Backoff)
            }
            Self::Control => {
                ("communication.control_failed", ErrorCategory::Unavailable, RetryAdvice::Backoff)
            }
            Self::Inbound => {
                ("communication.inbound_invalid", ErrorCategory::InvalidInput, RetryAdvice::Never)
            }
            Self::Attachment => (
                "communication.attachment_unavailable",
                ErrorCategory::Unavailable,
                RetryAdvice::Backoff,
            ),
            Self::AttachmentStage(stage) => match stage {
                AttachmentFailureStage::AckTimeout => (
                    "communication.attachment_ack_timeout",
                    ErrorCategory::Unavailable,
                    RetryAdvice::Backoff,
                ),
                AttachmentFailureStage::PeerUnavailable => (
                    "communication.attachment_peer_unavailable",
                    ErrorCategory::Unavailable,
                    RetryAdvice::Backoff,
                ),
                AttachmentFailureStage::Integrity => (
                    "communication.attachment_integrity_failed",
                    ErrorCategory::Conflict,
                    RetryAdvice::Never,
                ),
                AttachmentFailureStage::Storage => (
                    "communication.attachment_storage_failed",
                    ErrorCategory::Internal,
                    RetryAdvice::Never,
                ),
                AttachmentFailureStage::Dependency => (
                    "communication.attachment_dependency_missing",
                    ErrorCategory::Unavailable,
                    RetryAdvice::Backoff,
                ),
                AttachmentFailureStage::Protocol | AttachmentFailureStage::Unknown => (
                    "communication.attachment_protocol_failed",
                    ErrorCategory::InvalidInput,
                    RetryAdvice::Never,
                ),
            },
            Self::ReadState => {
                ("communication.read_state_failed", ErrorCategory::Internal, RetryAdvice::Never)
            }
            Self::Relationship => {
                ("communication.relationship_failed", ErrorCategory::Conflict, RetryAdvice::Never)
            }
            Self::Engine => {
                ("communication.engine_failed", ErrorCategory::Internal, RetryAdvice::Never)
            }
        };
        ErrorDescriptor::new(ErrorCode::new(code), category, retry)
    }
}
