//! Read-only projections built from domain snapshots.

use torca_contacts::{Contact, ContactId};
use torca_conversations::{ConversationId, ConversationStatus, DirectConversation};
use torca_messaging::{Message, MessageDirection, MessageId, MessageStatus};

/// Presentation-safe message summary.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageSummary { pub id: MessageId, pub text: String, pub direction: MessageDirection, pub status: MessageStatus }
/// Presentation-safe direct conversation view.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationView { pub id: ConversationId, pub contact_id: ContactId, pub status: ConversationStatus, pub messages: Vec<MessageSummary> }
/// Projection error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionError { ContactMissing }
/// Builds immutable views without owning workflow state.
pub struct ProjectionBuilder;
impl ProjectionBuilder {
    /// Builds one direct-conversation view and verifies contact ownership.
    pub fn conversation(conversation: &DirectConversation, contacts: &[Contact], messages: &[Message]) -> Result<ConversationView, ProjectionError> {
        if !contacts.iter().any(|contact| contact.id() == conversation.contact_id()) { return Err(ProjectionError::ContactMissing); }
        let messages = messages.iter().filter(|message| message.conversation_id() == conversation.id()).map(|message| MessageSummary { id: message.id(), text: message.body().as_str().to_owned(), direction: message.direction(), status: message.status() }).collect();
        Ok(ConversationView { id: conversation.id(), contact_id: conversation.contact_id(), status: conversation.status(), messages })
    }
}
