use std::collections::BTreeMap;
use std::path::PathBuf;

use torca_attachments::AttachmentId;
use torca_communication_driver::{CommunicationError, RelationshipAdminRuntime};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_crypto::ProtectedSecretStore;
use torca_file_storage::{BlobStore, FileBlobStore};
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::KeyId;
use torca_storage_sqlite::SqlCipherRelationshipAdmin;

pub struct RelationshipAdminAdapter<P> {
    store: SqlCipherRelationshipAdmin,
    peer_secrets: P,
    cache: FileBlobStore,
    staging_root: PathBuf,
}
impl<P> RelationshipAdminAdapter<P> {
    pub const fn new(
        store: SqlCipherRelationshipAdmin,
        peer_secrets: P,
        cache: FileBlobStore,
        staging_root: PathBuf,
    ) -> Self {
        Self { store, peer_secrets, cache, staging_root }
    }

    fn purge_attachments(&mut self, ids: &[OpaqueId]) -> Result<(), CommunicationError> {
        for id in ids {
            let attachment_id = AttachmentId::from_opaque(*id);
            self.cache.remove(attachment_id).map_err(|_| CommunicationError::Relationship)?;
            let path = self.staging_root.join(attachment_id.to_string());
            match std::fs::remove_dir_all(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(CommunicationError::Relationship),
            }
        }
        Ok(())
    }
}
impl<P> RelationshipAdminRuntime for RelationshipAdminAdapter<P>
where
    P: ProtectedSecretStore + Send + 'static,
{
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, CommunicationError> {
        self.store.contact_names().map_err(|_| CommunicationError::Relationship)
    }

    fn rename_contact(
        &mut self,
        contact_id: ContactId,
        display_name: String,
        now: Timestamp,
    ) -> Result<(), CommunicationError> {
        self.store
            .rename_contact(contact_id, display_name, now)
            .map_err(|_| CommunicationError::Relationship)
    }

    fn block_contact(&mut self, contact_id: ContactId, now: Timestamp) -> Result<(), CommunicationError> {
        self.store.block_contact(contact_id, now).map_err(|_| CommunicationError::Relationship)
    }

    fn unblock_contact(&mut self, contact_id: ContactId, now: Timestamp) -> Result<(), CommunicationError> {
        self.store.unblock_contact(contact_id, now).map_err(|_| CommunicationError::Relationship)
    }

    fn clear_history(&mut self, conversation_id: ConversationId) -> Result<(), CommunicationError> {
        let cleanup = self.store.clear_conversation_history(conversation_id)
            .map_err(|_| CommunicationError::Relationship)?;
        self.purge_attachments(&cleanup.attachment_ids)
    }

    fn remove_contact(&mut self, contact_id: ContactId) -> Result<(), CommunicationError> {
        let cleanup = self.store.remove_contact(contact_id)
            .map_err(|_| CommunicationError::Relationship)?;
        self.purge_attachments(&cleanup.attachment_ids)?;
        if let Some(handle) = cleanup.peer_secret_handle {
            let _ = self.peer_secrets.delete(KeyId::from_opaque(handle));
        }
        Ok(())
    }
}
