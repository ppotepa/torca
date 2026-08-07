use std::collections::BTreeMap;

use torca_attachment_transfer::AttachmentTransferStore;
use torca_communication_driver::{CommunicationError, RelationshipAdminRuntime};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_crypto::ProtectedSecretStore;
use torca_foundation::Timestamp;
use torca_runtime_host::ContactVerificationSnapshot;
use torca_storage_sqlite::SqlCipherRelationshipAdmin;

pub struct RelationshipAdminAdapter<S, A> {
    metadata: SqlCipherRelationshipAdmin,
    peer_secrets: S,
    attachments: A,
}
impl<S, A> RelationshipAdminAdapter<S, A> {
    pub const fn new(metadata: SqlCipherRelationshipAdmin, peer_secrets: S, attachments: A) -> Self {
        Self { metadata, peer_secrets, attachments }
    }
}
impl<S: ProtectedSecretStore + Send, A: AttachmentTransferStore + Send> RelationshipAdminRuntime
    for RelationshipAdminAdapter<S, A>
{
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, CommunicationError> {
        self.metadata.contact_names().map_err(|_| CommunicationError::Relationship)
    }

    fn contact_verifications(
        &self,
    ) -> Result<BTreeMap<ContactId, ContactVerificationSnapshot>, CommunicationError> {
        self.metadata
            .contact_verifications()
            .map(|values| {
                values
                    .into_iter()
                    .map(|(id, (verified, verified_at))| {
                        (id, ContactVerificationSnapshot { verified, verified_at })
                    })
                    .collect()
            })
            .map_err(|_| CommunicationError::Relationship)
    }

    fn verify_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.metadata
            .verify_contact(contact_id, now)
            .map_err(|_| CommunicationError::Relationship)
    }

    fn reset_contact_verification(
        &mut self,
        contact_id: ContactId,
    ) -> Result<(), CommunicationError> {
        self.metadata
            .reset_contact_verification(contact_id)
            .map_err(|_| CommunicationError::Relationship)
    }

    fn rename_contact(
        &mut self,
        contact_id: ContactId,
        display_name: String,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.metadata
            .rename_contact(contact_id, display_name, now)
            .map_err(|_| CommunicationError::Relationship)
    }

    fn block_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.metadata
            .block_contact(contact_id, now)
            .map_err(|_| CommunicationError::Relationship)
    }

    fn unblock_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.metadata
            .unblock_contact(contact_id, now)
            .map_err(|_| CommunicationError::Relationship)
    }

    fn clear_history(&mut self, conversation_id: ConversationId) -> Result<(), CommunicationError> {
        let cleanup = self
            .metadata
            .clear_conversation_history(conversation_id)
            .map_err(|_| CommunicationError::Relationship)?;
        for attachment_id in cleanup.attachment_ids {
            self.attachments
                .delete_attachment(attachment_id)
                .map_err(|_| CommunicationError::Attachment)?;
        }
        Ok(())
    }

    fn remove_contact(&mut self, contact_id: ContactId) -> Result<(), CommunicationError> {
        let cleanup = self
            .metadata
            .remove_contact(contact_id)
            .map_err(|_| CommunicationError::Relationship)?;
        for attachment_id in cleanup.attachment_ids {
            self.attachments
                .delete_attachment(attachment_id)
                .map_err(|_| CommunicationError::Attachment)?;
        }
        if let Some(handle) = cleanup.peer_secret_handle {
            self.peer_secrets
                .delete(torca_identity::KeyId::from_opaque(handle))
                .map_err(|_| CommunicationError::Relationship)?;
        }
        Ok(())
    }
}
