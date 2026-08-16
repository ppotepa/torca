//! Strict versioned attachment transfer payloads carried inside encrypted PeerMessage::Data.

use core::fmt;

use torca_attachments::{AttachmentId, AttachmentName, MAX_ATTACHMENT_BYTES, MediaType};
use torca_foundation::OpaqueId;

const MAGIC: &[u8; 4] = b"TCAT";
// Metadata gained a bounded visual preview in version 2; v3 adds an explicit
// cancel frame so a receiver can release durable temporary chunks immediately.
// Attachment peers negotiate one product protocol version, so rejecting an
// older wire is safer than silently treating cancellation as a timeout.
const VERSION: u16 = 3;
pub const MAX_ATTACHMENT_CHUNK: usize = 64 * 1024;
pub const MAX_ATTACHMENT_PREVIEW: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentFrameKind {
    Metadata = 1,
    Chunk = 2,
    Resume = 3,
    Complete = 4,
    Cancel = 5,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentMetadataFrame {
    pub attachment_id: AttachmentId,
    pub message_id: OpaqueId,
    pub name: AttachmentName,
    pub media_type: MediaType,
    pub size: u64,
    pub digest: [u8; 32],
    pub preview: Option<AttachmentPreviewFrame>,
}

/// A small visual representation sent with metadata, ahead of the full
/// chunked payload.  It lets a receiver render an image/poster while the
/// durable attachment job continues in the background.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentPreviewFrame {
    pub media_type: MediaType,
    pub bytes: Vec<u8>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentChunkFrame {
    pub attachment_id: AttachmentId,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentResumeFrame {
    pub attachment_id: AttachmentId,
    pub offset: u64,
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentCompleteFrame {
    pub attachment_id: AttachmentId,
    pub digest: [u8; 32],
}

/// Cancels an in-flight transfer without deleting another attachment that may
/// share the same conversation. The attachment id is the entire authority.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentCancelFrame {
    pub attachment_id: AttachmentId,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachmentFrame {
    Metadata(AttachmentMetadataFrame),
    Chunk(AttachmentChunkFrame),
    Resume(AttachmentResumeFrame),
    Complete(AttachmentCompleteFrame),
    Cancel(AttachmentCancelFrame),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachmentProtocolError {
    InvalidMagic,
    UnsupportedVersion(u16),
    UnknownKind(u8),
    InvalidMetadata,
    ChunkTooLarge,
    InvalidOffset,
    InvalidUtf8,
    Truncated,
    TrailingBytes,
}
impl fmt::Display for AttachmentProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AttachmentProtocolError {}

pub struct AttachmentCodec;
impl AttachmentCodec {
    pub fn encode(frame: &AttachmentFrame) -> Result<Vec<u8>, AttachmentProtocolError> {
        let mut output = Vec::new();
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&VERSION.to_be_bytes());
        match frame {
            AttachmentFrame::Metadata(metadata) => {
                validate_metadata(metadata)?;
                output.push(AttachmentFrameKind::Metadata as u8);
                output.extend_from_slice(metadata.attachment_id.to_opaque().as_bytes());
                output.extend_from_slice(metadata.message_id.as_bytes());
                put_bytes(metadata.name.as_str().as_bytes(), &mut output)?;
                put_bytes(metadata.media_type.as_str().as_bytes(), &mut output)?;
                output.extend_from_slice(&metadata.size.to_be_bytes());
                output.extend_from_slice(&metadata.digest);
                match &metadata.preview {
                    Some(preview) => {
                        validate_preview(preview)?;
                        output.push(1);
                        put_bytes(preview.media_type.as_str().as_bytes(), &mut output)?;
                        output.extend_from_slice(&(preview.bytes.len() as u32).to_be_bytes());
                        output.extend_from_slice(&preview.bytes);
                    }
                    None => output.push(0),
                }
            }
            AttachmentFrame::Chunk(chunk) => {
                validate_chunk(chunk)?;
                output.push(AttachmentFrameKind::Chunk as u8);
                output.extend_from_slice(chunk.attachment_id.to_opaque().as_bytes());
                output.extend_from_slice(&chunk.offset.to_be_bytes());
                output.extend_from_slice(&(chunk.bytes.len() as u32).to_be_bytes());
                output.extend_from_slice(&chunk.bytes);
            }
            AttachmentFrame::Resume(resume) => {
                output.push(AttachmentFrameKind::Resume as u8);
                output.extend_from_slice(resume.attachment_id.to_opaque().as_bytes());
                output.extend_from_slice(&resume.offset.to_be_bytes());
            }
            AttachmentFrame::Complete(complete) => {
                output.push(AttachmentFrameKind::Complete as u8);
                output.extend_from_slice(complete.attachment_id.to_opaque().as_bytes());
                output.extend_from_slice(&complete.digest);
            }
            AttachmentFrame::Cancel(cancel) => {
                output.push(AttachmentFrameKind::Cancel as u8);
                output.extend_from_slice(cancel.attachment_id.to_opaque().as_bytes());
            }
        }
        Ok(output)
    }

    pub fn decode(input: &[u8]) -> Result<AttachmentFrame, AttachmentProtocolError> {
        let mut cursor = Cursor::new(input);
        if cursor.take(4)? != MAGIC {
            return Err(AttachmentProtocolError::InvalidMagic);
        }
        let version = cursor.u16()?;
        if version != VERSION {
            return Err(AttachmentProtocolError::UnsupportedVersion(version));
        }
        let frame = match cursor.u8()? {
            1 => {
                let attachment_id = AttachmentId::from_opaque(cursor.id()?);
                let message_id = cursor.id()?;
                let name = String::from_utf8(cursor.bytes(AttachmentName::MAX_BYTES)?)
                    .map_err(|_| AttachmentProtocolError::InvalidUtf8)?;
                let media = String::from_utf8(cursor.bytes(MediaType::MAX_BYTES)?)
                    .map_err(|_| AttachmentProtocolError::InvalidUtf8)?;
                let size = cursor.u64()?;
                let digest = cursor.array32()?;
                let preview = match cursor.u8()? {
                    0 => None,
                    1 => {
                        let media_type = String::from_utf8(cursor.bytes(MediaType::MAX_BYTES)?)
                            .map_err(|_| AttachmentProtocolError::InvalidUtf8)?;
                        let length = usize::try_from(cursor.u32()?)
                            .map_err(|_| AttachmentProtocolError::InvalidMetadata)?;
                        if length == 0 || length > MAX_ATTACHMENT_PREVIEW {
                            return Err(AttachmentProtocolError::InvalidMetadata);
                        }
                        Some(AttachmentPreviewFrame {
                            media_type: MediaType::new(media_type)
                                .map_err(|_| AttachmentProtocolError::InvalidMetadata)?,
                            bytes: cursor.take(length)?.to_vec(),
                        })
                    }
                    _ => return Err(AttachmentProtocolError::InvalidMetadata),
                };
                let metadata = AttachmentMetadataFrame {
                    attachment_id,
                    message_id,
                    name: AttachmentName::new(name)
                        .map_err(|_| AttachmentProtocolError::InvalidMetadata)?,
                    media_type: MediaType::new(media)
                        .map_err(|_| AttachmentProtocolError::InvalidMetadata)?,
                    size,
                    digest,
                    preview,
                };
                validate_metadata(&metadata)?;
                AttachmentFrame::Metadata(metadata)
            }
            2 => {
                let attachment_id = AttachmentId::from_opaque(cursor.id()?);
                let offset = cursor.u64()?;
                let length = usize::try_from(cursor.u32()?)
                    .map_err(|_| AttachmentProtocolError::ChunkTooLarge)?;
                if length == 0 || length > MAX_ATTACHMENT_CHUNK {
                    return Err(AttachmentProtocolError::ChunkTooLarge);
                }
                let chunk = AttachmentChunkFrame {
                    attachment_id,
                    offset,
                    bytes: cursor.take(length)?.to_vec(),
                };
                validate_chunk(&chunk)?;
                AttachmentFrame::Chunk(chunk)
            }
            3 => AttachmentFrame::Resume(AttachmentResumeFrame {
                attachment_id: AttachmentId::from_opaque(cursor.id()?),
                offset: cursor.u64()?,
            }),
            4 => AttachmentFrame::Complete(AttachmentCompleteFrame {
                attachment_id: AttachmentId::from_opaque(cursor.id()?),
                digest: cursor.array32()?,
            }),
            5 => AttachmentFrame::Cancel(AttachmentCancelFrame {
                attachment_id: AttachmentId::from_opaque(cursor.id()?),
            }),
            value => return Err(AttachmentProtocolError::UnknownKind(value)),
        };
        if !cursor.is_empty() {
            return Err(AttachmentProtocolError::TrailingBytes);
        }
        Ok(frame)
    }
}

fn validate_metadata(metadata: &AttachmentMetadataFrame) -> Result<(), AttachmentProtocolError> {
    if metadata.size == 0 || metadata.size > MAX_ATTACHMENT_BYTES {
        return Err(AttachmentProtocolError::InvalidMetadata);
    }
    if let Some(preview) = &metadata.preview {
        validate_preview(preview)?;
    }
    Ok(())
}

fn validate_preview(preview: &AttachmentPreviewFrame) -> Result<(), AttachmentProtocolError> {
    if preview.bytes.is_empty()
        || preview.bytes.len() > MAX_ATTACHMENT_PREVIEW
        || !preview.media_type.as_str().starts_with("image/")
    {
        return Err(AttachmentProtocolError::InvalidMetadata);
    }
    Ok(())
}
fn validate_chunk(chunk: &AttachmentChunkFrame) -> Result<(), AttachmentProtocolError> {
    if chunk.bytes.is_empty() || chunk.bytes.len() > MAX_ATTACHMENT_CHUNK {
        return Err(AttachmentProtocolError::ChunkTooLarge);
    }
    let length =
        u64::try_from(chunk.bytes.len()).map_err(|_| AttachmentProtocolError::ChunkTooLarge)?;
    if chunk.offset.checked_add(length).is_none() {
        return Err(AttachmentProtocolError::InvalidOffset);
    }
    Ok(())
}
fn put_bytes(value: &[u8], output: &mut Vec<u8>) -> Result<(), AttachmentProtocolError> {
    let len = u16::try_from(value.len()).map_err(|_| AttachmentProtocolError::InvalidMetadata)?;
    output.extend_from_slice(&len.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], AttachmentProtocolError> {
        let end = self.offset.checked_add(len).ok_or(AttachmentProtocolError::Truncated)?;
        let value = self.input.get(self.offset..end).ok_or(AttachmentProtocolError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, AttachmentProtocolError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, AttachmentProtocolError> {
        Ok(u16::from_be_bytes(
            self.take(2)?.try_into().map_err(|_| AttachmentProtocolError::Truncated)?,
        ))
    }
    fn u32(&mut self) -> Result<u32, AttachmentProtocolError> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().map_err(|_| AttachmentProtocolError::Truncated)?,
        ))
    }
    fn u64(&mut self) -> Result<u64, AttachmentProtocolError> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| AttachmentProtocolError::Truncated)?,
        ))
    }
    fn id(&mut self) -> Result<OpaqueId, AttachmentProtocolError> {
        Ok(OpaqueId::from_bytes(
            self.take(16)?.try_into().map_err(|_| AttachmentProtocolError::Truncated)?,
        ))
    }
    fn array32(&mut self) -> Result<[u8; 32], AttachmentProtocolError> {
        self.take(32)?.try_into().map_err(|_| AttachmentProtocolError::Truncated)
    }
    fn bytes(&mut self, maximum: usize) -> Result<Vec<u8>, AttachmentProtocolError> {
        let length = usize::from(self.u16()?);
        if length > maximum {
            return Err(AttachmentProtocolError::InvalidMetadata);
        }
        Ok(self.take(length)?.to_vec())
    }
    const fn is_empty(&self) -> bool {
        self.offset == self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_round_trips_without_losing_message_binding() {
        let frame = AttachmentFrame::Metadata(AttachmentMetadataFrame {
            attachment_id: AttachmentId::from_u128(1),
            message_id: OpaqueId::from_u128(2),
            name: AttachmentName::new("photo.jpg").expect("valid name"),
            media_type: MediaType::new("image/jpeg").expect("valid media type"),
            size: 3,
            digest: [7; 32],
            preview: Some(AttachmentPreviewFrame {
                media_type: MediaType::new("image/jpeg").expect("valid preview media type"),
                bytes: vec![1, 2, 3],
            }),
        });
        let encoded = AttachmentCodec::encode(&frame).expect("encode succeeds");
        assert_eq!(AttachmentCodec::decode(&encoded).expect("decode succeeds"), frame);
    }

    #[test]
    fn oversized_chunk_is_rejected_before_transport() {
        let frame = AttachmentFrame::Chunk(AttachmentChunkFrame {
            attachment_id: AttachmentId::from_u128(1),
            offset: 0,
            bytes: vec![0; MAX_ATTACHMENT_CHUNK + 1],
        });
        assert_eq!(AttachmentCodec::encode(&frame), Err(AttachmentProtocolError::ChunkTooLarge));
    }

    #[test]
    fn resume_complete_and_cancel_frames_round_trip() {
        let attachment_id = AttachmentId::from_u128(9);
        let frames = [
            AttachmentFrame::Resume(AttachmentResumeFrame { attachment_id, offset: 128 }),
            AttachmentFrame::Complete(AttachmentCompleteFrame { attachment_id, digest: [4; 32] }),
            AttachmentFrame::Cancel(AttachmentCancelFrame { attachment_id }),
        ];

        for frame in frames {
            let encoded = AttachmentCodec::encode(&frame).expect("encode succeeds");
            assert_eq!(AttachmentCodec::decode(&encoded).expect("decode succeeds"), frame);
        }
    }
}
