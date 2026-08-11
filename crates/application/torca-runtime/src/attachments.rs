use torca_foundation::OpaqueId;

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentSendRequest {
    pub attachment_id: OpaqueId,
    pub message_id: OpaqueId,
    pub conversation_id: OpaqueId,
    pub source_path: String,
    pub name: String,
    pub media_type: String,
    pub size: u64,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentView {
    pub id: OpaqueId,
    pub message_id: OpaqueId,
    pub name: String,
    pub media_type: String,
    pub size: u64,
    pub status: String,
    pub offset: u64,
    pub attempt_count: u32,
    pub updated_at_ms: i64,
    pub direction: String,
}
