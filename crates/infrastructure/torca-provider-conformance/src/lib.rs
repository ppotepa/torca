//! Executable provider contract shared by the production Iroh provider and
//! the deterministic Memory test double.

use torca_foundation::ProviderId;
use torca_transport_api::{TransportPath, TransportTopology};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConformanceProvider {
    Memory,
    Iroh,
}

impl ConformanceProvider {
    #[allow(clippy::missing_panics_doc)]
    pub fn provider_id(self) -> ProviderId {
        ProviderId::new(match self {
            Self::Memory => "memory",
            Self::Iroh => "iroh",
        })
        .expect("static provider id")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceRoute {
    pub path: TransportPath,
    pub persisted: bool,
}

pub fn persisted_route(
    provider: ConformanceProvider,
    topology: TransportTopology,
) -> ConformanceRoute {
    ConformanceRoute {
        path: TransportPath { provider: provider.provider_id(), topology },
        persisted: true,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use tokio::runtime::Runtime;
    use torca_client_engine::{
        AvatarGenomeRecord, ClientEngine, ClientEngineActor, EngineCommand, EngineError,
        InMemoryRelationshipRepository, RelationshipRepository,
    };
    use torca_contacts::{
        Contact, ContactError, ContactId, ContactRepository, PeerCredential,
        PeerCredentialRepository,
    };
    use torca_conversations::{
        ConversationError, ConversationId, ConversationRepository, DirectConversation,
    };
    use torca_crypto::{
        InMemoryProtectedSecretStore, ManagedIdentityKeys, ProtectedSecretStore,
        ProtectedSecretStoreError, RustCryptoProvider, RustPairingCrypto,
    };
    use torca_foundation::{OpaqueId, Timestamp};
    use torca_identity::{
        IdentityId, InMemoryIdentityRepository, KeyId, Profile, ProfileName, PublicIdentity,
    };
    use torca_messaging::InMemoryMessageRepository;
    use torca_pairing::{InMemoryPairingRepository, PairingCode, PairingSessionId};
    use torca_pairing_coordinator::{
        LocalPairingContext, PairingCoordinator, PairingCoordinatorError, PairingCredentialError,
        PairingDerivedSecret, PairingPeerSecretStore, PairingRuntime, PairingSessionDelivery,
        PairingSessionServicePort, PairingSideToken, PairingSlotCapability, PairingSlotId,
    };
    use torca_peer_link::{LinkAck, PeerConnectionState, PeerLink};
    use torca_peer_protocol::{AckStatus, HandshakeSigner, HandshakeSigningError};
    use torca_receipts::InMemoryReceiptRepository;
    use torca_runtime::CommunicationLifecycle;
    use torca_transport_api::{PeerTransportFactory, TransportFactoryError};
    use torca_transport_iroh::{IrohComposition, IrohEndpointProfile, IrohLifecycle};
    use torca_transport_memory::MemoryNetwork;

    use super::*;

    const TEXT_KIND: u16 = 1;

    #[derive(Clone, Default)]
    struct SharedSecretStore(Arc<Mutex<InMemoryProtectedSecretStore>>);

    impl ProtectedSecretStore for SharedSecretStore {
        fn insert(
            &mut self,
            key_id: KeyId,
            secret: &[u8],
        ) -> Result<(), ProtectedSecretStoreError> {
            self.0.lock().expect("secret store lock").insert(key_id, secret)
        }

        fn load(&self, key_id: KeyId) -> Result<Option<Vec<u8>>, ProtectedSecretStoreError> {
            self.0.lock().expect("secret store lock").load(key_id)
        }

        fn delete(&mut self, key_id: KeyId) -> Result<bool, ProtectedSecretStoreError> {
            self.0.lock().expect("secret store lock").delete(key_id)
        }
    }

    #[derive(Clone, Default)]
    struct SharedRelationships(Arc<Mutex<InMemoryRelationshipRepository>>);

    impl SharedRelationships {
        fn peer_snapshot(&self) -> PersistedRelationships {
            let relationships = self.0.lock().expect("relationship store lock");
            let contacts = ContactRepository::list(&*relationships).expect("persisted contacts");
            let credentials = contacts
                .iter()
                .map(|contact| {
                    let credential = PeerCredentialRepository::credential_for_contact(
                        &*relationships,
                        contact.id(),
                    )
                    .expect("persisted credential query")
                    .expect("persisted pairing credential");
                    (contact.id(), credential)
                })
                .collect();
            PersistedRelationships {
                contacts: contacts.into_iter().map(|contact| (contact.id(), contact)).collect(),
                credentials,
            }
        }
    }

    impl ContactRepository for SharedRelationships {
        fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
            ContactRepository::insert(
                &mut *self.0.lock().expect("relationship store lock"),
                contact,
            )
        }

        fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
            ContactRepository::get(&*self.0.lock().expect("relationship store lock"), id)
        }

        fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
            ContactRepository::update(
                &mut *self.0.lock().expect("relationship store lock"),
                contact,
            )
        }

        fn list(&self) -> Result<Vec<Contact>, ContactError> {
            ContactRepository::list(&*self.0.lock().expect("relationship store lock"))
        }
    }

    impl PeerCredentialRepository for SharedRelationships {
        fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
            PeerCredentialRepository::insert_credential(
                &mut *self.0.lock().expect("relationship store lock"),
                credential,
            )
        }

        fn credential_for_contact(
            &self,
            contact_id: ContactId,
        ) -> Result<Option<PeerCredential>, ContactError> {
            PeerCredentialRepository::credential_for_contact(
                &*self.0.lock().expect("relationship store lock"),
                contact_id,
            )
        }
    }

    impl ConversationRepository for SharedRelationships {
        fn insert(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
            ConversationRepository::insert(
                &mut *self.0.lock().expect("relationship store lock"),
                conversation,
            )
        }

        fn get(&self, id: ConversationId) -> Result<Option<DirectConversation>, ConversationError> {
            ConversationRepository::get(&*self.0.lock().expect("relationship store lock"), id)
        }

        fn for_contact(
            &self,
            contact_id: ContactId,
        ) -> Result<Option<DirectConversation>, ConversationError> {
            ConversationRepository::for_contact(
                &*self.0.lock().expect("relationship store lock"),
                contact_id,
            )
        }

        fn list(&self) -> Result<Vec<DirectConversation>, ConversationError> {
            ConversationRepository::list(&*self.0.lock().expect("relationship store lock"))
        }

        fn update(&mut self, conversation: DirectConversation) -> Result<(), ConversationError> {
            ConversationRepository::update(
                &mut *self.0.lock().expect("relationship store lock"),
                conversation,
            )
        }
    }

    impl RelationshipRepository for SharedRelationships {
        fn upsert_avatar_genome(
            &mut self,
            record: AvatarGenomeRecord,
            at: Timestamp,
        ) -> Result<(), EngineError> {
            self.0.lock().expect("relationship store lock").upsert_avatar_genome(record, at)
        }

        fn avatar_genome(&self, hash: [u8; 32]) -> Result<Option<AvatarGenomeRecord>, EngineError> {
            self.0.lock().expect("relationship store lock").avatar_genome(hash)
        }

        fn avatar_genome_for_identity(
            &self,
            identity_id: IdentityId,
        ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
            self.0.lock().expect("relationship store lock").avatar_genome_for_identity(identity_id)
        }

        fn local_avatar_genome(&self) -> Result<Option<AvatarGenomeRecord>, EngineError> {
            self.0.lock().expect("relationship store lock").local_avatar_genome()
        }

        fn insert_pairing_result(
            &mut self,
            contact: Contact,
            conversation: DirectConversation,
            display_name: &str,
            credential: PeerCredential,
            avatar: Option<AvatarGenomeRecord>,
            at: Timestamp,
        ) -> Result<(), EngineError> {
            self.0.lock().expect("relationship store lock").insert_pairing_result(
                contact,
                conversation,
                display_name,
                credential,
                avatar,
                at,
            )
        }

        fn remove_relationship(&mut self, contact_id: ContactId) -> Result<(), EngineError> {
            self.0.lock().expect("relationship store lock").remove_relationship(contact_id)
        }
    }

    #[derive(Default)]
    struct PairingSecrets {
        next: u128,
        pairing_states: BTreeMap<PairingSessionId, Vec<u8>>,
    }

    impl PairingPeerSecretStore for PairingSecrets {
        fn store_peer_secret(
            &mut self,
            _secret: PairingDerivedSecret,
        ) -> Result<OpaqueId, PairingCredentialError> {
            self.next += 1;
            Ok(OpaqueId::from_u128(self.next))
        }

        fn delete_peer_secret(
            &mut self,
            _handle: OpaqueId,
        ) -> Result<bool, PairingCredentialError> {
            Ok(true)
        }

        fn store_pairing_state(
            &mut self,
            session_id: PairingSessionId,
            state: &[u8],
        ) -> Result<(), PairingCredentialError> {
            self.pairing_states.insert(session_id, state.to_vec());
            Ok(())
        }

        fn load_pairing_state(
            &self,
            session_id: PairingSessionId,
        ) -> Result<Option<Vec<u8>>, PairingCredentialError> {
            Ok(self.pairing_states.get(&session_id).cloned())
        }

        fn delete_pairing_state(
            &mut self,
            session_id: PairingSessionId,
        ) -> Result<bool, PairingCredentialError> {
            Ok(self.pairing_states.remove(&session_id).is_some())
        }
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    enum PairingSide {
        Creator,
        Joiner,
    }

    struct PairingDelivery {
        sequence: u64,
        sender: PairingSide,
        blob: Vec<u8>,
    }

    struct PairingSlot {
        id: PairingSlotId,
        code: String,
        expires_at: Timestamp,
        capability: PairingSlotCapability,
        creator_token: PairingSideToken,
        joiner_token: Option<PairingSideToken>,
        creator_blob: Vec<u8>,
        deliveries: Vec<PairingDelivery>,
    }

    #[derive(Default)]
    struct PairingBroker {
        next: u128,
        slots: Vec<PairingSlot>,
    }

    #[derive(Clone, Default)]
    struct MemoryPairingService(Arc<Mutex<PairingBroker>>);

    impl MemoryPairingService {
        fn side(slot: &PairingSlot, token: PairingSideToken) -> Option<PairingSide> {
            if slot.creator_token == token {
                Some(PairingSide::Creator)
            } else if slot.joiner_token == Some(token) {
                Some(PairingSide::Joiner)
            } else {
                None
            }
        }
    }

    impl PairingSessionServicePort for MemoryPairingService {
        fn open(
            &mut self,
            code: &PairingCode,
            expires_at: Timestamp,
            creator_blob: Vec<u8>,
            capability: PairingSlotCapability,
            creator_token: PairingSideToken,
            _ticket: [u8; 16],
        ) -> Result<(PairingSlotId, Timestamp), PairingCoordinatorError> {
            let mut broker = self.0.lock().map_err(|_| PairingCoordinatorError::SessionService)?;
            broker.next += 1;
            let id = PairingSlotId(OpaqueId::from_u128(broker.next));
            broker.slots.push(PairingSlot {
                id,
                code: code.as_str().to_owned(),
                expires_at,
                capability,
                creator_token,
                joiner_token: None,
                creator_blob,
                deliveries: Vec::new(),
            });
            Ok((id, expires_at))
        }

        fn join(
            &mut self,
            code: &PairingCode,
            joiner_blob: Vec<u8>,
            joiner_token: PairingSideToken,
            _ticket: Option<[u8; 16]>,
            _bootstrap: Option<&torca_pairing_protocol::PairingBootstrapDescriptor>,
        ) -> Result<(PairingSlotId, Timestamp, Vec<u8>), PairingCoordinatorError> {
            let mut broker = self.0.lock().map_err(|_| PairingCoordinatorError::SessionService)?;
            let slot = broker
                .slots
                .iter_mut()
                .find(|slot| slot.code == code.as_str())
                .ok_or(PairingCoordinatorError::SessionNotFound)?;
            slot.joiner_token = Some(joiner_token);
            let sequence = u64::try_from(slot.deliveries.len() + 1)
                .map_err(|_| PairingCoordinatorError::SessionService)?;
            slot.deliveries.push(PairingDelivery {
                sequence,
                sender: PairingSide::Joiner,
                blob: joiner_blob,
            });
            Ok((slot.id, slot.expires_at, slot.creator_blob.clone()))
        }

        fn push(
            &mut self,
            _message_id: OpaqueId,
            slot_id: PairingSlotId,
            token: PairingSideToken,
            blob: Vec<u8>,
        ) -> Result<(), PairingCoordinatorError> {
            let mut broker = self.0.lock().map_err(|_| PairingCoordinatorError::SessionService)?;
            let slot = broker
                .slots
                .iter_mut()
                .find(|slot| slot.id == slot_id)
                .ok_or(PairingCoordinatorError::SessionNotFound)?;
            let sender = Self::side(slot, token).ok_or(PairingCoordinatorError::SessionService)?;
            let sequence = u64::try_from(slot.deliveries.len() + 1)
                .map_err(|_| PairingCoordinatorError::SessionService)?;
            slot.deliveries.push(PairingDelivery { sequence, sender, blob });
            Ok(())
        }

        fn poll(
            &mut self,
            slot_id: PairingSlotId,
            token: PairingSideToken,
            after: u64,
        ) -> Result<Vec<PairingSessionDelivery>, PairingCoordinatorError> {
            let broker = self.0.lock().map_err(|_| PairingCoordinatorError::SessionService)?;
            let slot = broker
                .slots
                .iter()
                .find(|slot| slot.id == slot_id)
                .ok_or(PairingCoordinatorError::SessionNotFound)?;
            let recipient =
                Self::side(slot, token).ok_or(PairingCoordinatorError::SessionService)?;
            Ok(slot
                .deliveries
                .iter()
                .filter(|delivery| delivery.sequence > after && delivery.sender != recipient)
                .map(|delivery| PairingSessionDelivery {
                    sequence: delivery.sequence,
                    blob: delivery.blob.clone(),
                })
                .collect())
        }

        fn ack(
            &mut self,
            _slot: PairingSlotId,
            _token: PairingSideToken,
            _up_to: u64,
        ) -> Result<(), PairingCoordinatorError> {
            Ok(())
        }

        fn close(
            &mut self,
            slot_id: PairingSlotId,
            capability: PairingSlotCapability,
        ) -> Result<(), PairingCoordinatorError> {
            let mut broker = self.0.lock().map_err(|_| PairingCoordinatorError::SessionService)?;
            let index = broker
                .slots
                .iter()
                .position(|slot| slot.id == slot_id && slot.capability == capability)
                .ok_or(PairingCoordinatorError::SessionNotFound)?;
            broker.slots.remove(index);
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct PersistedRelationships {
        contacts: BTreeMap<ContactId, Contact>,
        credentials: BTreeMap<ContactId, PeerCredential>,
    }

    impl PersistedRelationships {
        fn refresh_endpoint(
            &mut self,
            contact_id: ContactId,
            provider: &ProviderId,
            endpoint: Vec<u8>,
        ) {
            let contact = self.contacts.get_mut(&contact_id).expect("persisted contact");
            let mut route = contact.route().clone();
            route
                .update_provider_endpoint(provider.as_str(), endpoint)
                .expect("refresh provider endpoint");
            contact.update_route(route, current_timestamp()).expect("persist refreshed route");
        }
    }

    impl ContactRepository for PersistedRelationships {
        fn insert(&mut self, contact: Contact) -> Result<(), ContactError> {
            if self.contacts.insert(contact.id(), contact).is_some() {
                return Err(ContactError::AlreadyExists);
            }
            Ok(())
        }

        fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
            Ok(self.contacts.get(&id).cloned())
        }

        fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
            if !self.contacts.contains_key(&contact.id()) {
                return Err(ContactError::NotFound);
            }
            self.contacts.insert(contact.id(), contact);
            Ok(())
        }

        fn list(&self) -> Result<Vec<Contact>, ContactError> {
            Ok(self.contacts.values().cloned().collect())
        }
    }

    impl PeerCredentialRepository for PersistedRelationships {
        fn insert_credential(&mut self, credential: PeerCredential) -> Result<(), ContactError> {
            if self.credentials.insert(credential.contact_id(), credential).is_some() {
                return Err(ContactError::AlreadyExists);
            }
            Ok(())
        }

        fn credential_for_contact(
            &self,
            contact_id: ContactId,
        ) -> Result<Option<PeerCredential>, ContactError> {
            Ok(self.credentials.get(&contact_id).copied())
        }
    }

    type IdentityKeys = ManagedIdentityKeys<RustCryptoProvider, SharedSecretStore>;

    #[derive(Clone)]
    struct SharedSigner {
        keys: Arc<IdentityKeys>,
        key_id: KeyId,
    }

    impl HandshakeSigner for SharedSigner {
        fn sign(&self, canonical: &[u8]) -> Result<Vec<u8>, HandshakeSigningError> {
            self.keys
                .sign(self.key_id, canonical)
                .map(|signature| signature.0.to_vec())
                .map_err(|error| HandshakeSigningError(error.to_string()))
        }
    }

    struct PairingNode {
        engine: torca_client_engine::EngineHandle,
        actor: ClientEngineActor,
        relationships: SharedRelationships,
        secret_store: SharedSecretStore,
        public_identity: PublicIdentity,
        signer: SharedSigner,
    }

    fn pairing_node(identity_id: IdentityId, name: &str, now: Timestamp) -> PairingNode {
        let secret_store = SharedSecretStore::default();
        let relationships = SharedRelationships::default();
        let engine = ClientEngine::new(
            InMemoryIdentityRepository::default(),
            ManagedIdentityKeys::new(RustCryptoProvider, secret_store.clone()),
            InMemoryPairingRepository::default(),
            relationships.clone(),
            InMemoryMessageRepository::default(),
            InMemoryReceiptRepository::default(),
        );
        let (engine, actor) = ClientEngineActor::spawn(engine);
        let _ = engine
            .dispatch(EngineCommand::CreateIdentity {
                identity_id,
                profile: Some(Profile::new(ProfileName::new(name).expect("profile name"), None)),
                at: now,
            })
            .expect("create conformance identity");
        let public_identity = engine
            .overview_snapshot()
            .expect("identity snapshot")
            .identity
            .expect("persisted identity")
            .public()
            .clone();
        let signer = SharedSigner {
            keys: Arc::new(ManagedIdentityKeys::new(RustCryptoProvider, secret_store.clone())),
            key_id: public_identity.key().key_id(),
        };
        PairingNode { engine, actor, relationships, secret_store, public_identity, signer }
    }

    struct PersistedPairingNode {
        relationships: PersistedRelationships,
        public_identity: PublicIdentity,
        signer: SharedSigner,
        contact_id: ContactId,
    }

    fn complete_pairing(
        provider: &ProviderId,
        first_endpoint: &[u8],
        second_endpoint: &[u8],
    ) -> (PersistedPairingNode, PersistedPairingNode) {
        let now = Timestamp::from_unix_millis(1_000).expect("pairing time");
        let first = pairing_node(IdentityId::from_u128(101), "Alice", now);
        let second = pairing_node(IdentityId::from_u128(202), "Bob", now);
        let pairing_service = MemoryPairingService::default();
        let first_session_id = PairingSessionId::from_u128(77);
        let second_session_id = PairingSessionId::from_u128(88);
        let mut first_pairing = PairingRuntime::new(
            PairingCoordinator::new(pairing_service.clone(), RustPairingCrypto::new()),
            first.engine.clone(),
            ManagedIdentityKeys::new(RustCryptoProvider, first.secret_store.clone()),
            PairingSecrets::default(),
        );
        let mut second_pairing = PairingRuntime::new(
            PairingCoordinator::new(pairing_service, RustPairingCrypto::new()),
            second.engine.clone(),
            ManagedIdentityKeys::new(RustCryptoProvider, second.secret_store.clone()),
            PairingSecrets::default(),
        );

        let invitation = first_pairing
            .create_invitation_pending_route(first_session_id, now)
            .expect("create conformance invitation");
        second_pairing
            .join_invitation_pending_route(
                second_session_id,
                invitation.code,
                Some(*invitation.ticket.as_bytes()),
            )
            .expect("join conformance invitation");
        first_pairing
            .publish_local_offer(
                first_session_id,
                LocalPairingContext {
                    public_identity: first.public_identity.clone(),
                    display_name: "Alice".into(),
                    country_code: None,
                    capability_id: OpaqueId::from_u128(1_001),
                    avatar: None,
                    transport_provider: provider.as_str().into(),
                    transport_endpoint: first_endpoint.to_vec(),
                },
            )
            .expect("publish first provider route");
        second_pairing
            .publish_local_offer(
                second_session_id,
                LocalPairingContext {
                    public_identity: second.public_identity.clone(),
                    display_name: "Bob".into(),
                    country_code: None,
                    capability_id: OpaqueId::from_u128(2_002),
                    avatar: None,
                    transport_provider: provider.as_str().into(),
                    transport_endpoint: second_endpoint.to_vec(),
                },
            )
            .expect("publish second provider route");

        let _ = first_pairing.poll(first_session_id, now).expect("first offer exchange");
        let _ = second_pairing.poll(second_session_id, now).expect("second offer and approval");
        let _ = first_pairing.poll(first_session_id, now).expect("first receives approval");
        first_pairing.approve(first_session_id, now).expect("first approval");
        let second_report =
            second_pairing.poll(second_session_id, now).expect("second persists completion");
        let first_report =
            first_pairing.poll(first_session_id, now).expect("first persists completion");
        let _ = second_pairing
            .poll(second_session_id, now)
            .expect("second receives completion acknowledgement");

        let first_contact_id =
            first_report.completed_contact.expect("first completed contact").contact_id;
        let second_contact_id =
            second_report.completed_contact.expect("second completed contact").contact_id;
        assert_eq!(first_contact_id, second_contact_id);
        for engine in [&first.engine, &second.engine] {
            let snapshot = engine.overview_snapshot().expect("post-pairing snapshot");
            assert_eq!(snapshot.contacts.len(), 1);
            assert_eq!(snapshot.conversations.len(), 1);
            assert!(snapshot.pairings.is_empty());
        }

        drop(first_pairing);
        drop(second_pairing);
        first.actor.shutdown().expect("shutdown first pairing engine");
        second.actor.shutdown().expect("shutdown second pairing engine");

        (
            PersistedPairingNode {
                relationships: first.relationships.peer_snapshot(),
                public_identity: first.public_identity,
                signer: first.signer,
                contact_id: first_contact_id,
            },
            PersistedPairingNode {
                relationships: second.relationships.peer_snapshot(),
                public_identity: second.public_identity,
                signer: second.signer,
                contact_id: second_contact_id,
            },
        )
    }

    struct ProviderPair {
        provider: ProviderId,
        first_factory: Box<dyn PeerTransportFactory>,
        second_factory: Box<dyn PeerTransportFactory>,
        first_endpoint: Vec<u8>,
        second_endpoint: Vec<u8>,
        lifecycles: Option<(IrohLifecycle, IrohLifecycle)>,
        runtime: Option<Arc<Runtime>>,
    }

    fn memory_pair(network: &MemoryNetwork, first: &[u8], second: &[u8]) -> ProviderPair {
        let first_factory = network.bind(first.to_vec()).expect("bind first Memory node");
        let second_factory = network.bind(second.to_vec()).expect("bind second Memory node");
        ProviderPair {
            provider: ConformanceProvider::Memory.provider_id(),
            first_endpoint: first_factory.endpoint().to_vec(),
            second_endpoint: second_factory.endpoint().to_vec(),
            first_factory: Box::new(first_factory),
            second_factory: Box::new(second_factory),
            lifecycles: None,
            runtime: None,
        }
    }

    fn iroh_pair() -> ProviderPair {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let mut first_store = InMemoryProtectedSecretStore::default();
        let mut second_store = InMemoryProtectedSecretStore::default();
        let first = IrohComposition::bind_with_profile(
            Arc::clone(&runtime),
            &mut first_store,
            IrohEndpointProfile::DirectOnly,
            false,
        )
        .expect("bind first Iroh node");
        let second = IrohComposition::bind_with_profile(
            Arc::clone(&runtime),
            &mut second_store,
            IrohEndpointProfile::DirectOnly,
            false,
        )
        .expect("bind second Iroh node");
        let first_endpoint = first.peer_endpoint_bytes().expect("first Iroh route");
        let second_endpoint = second.peer_endpoint_bytes().expect("second Iroh route");
        ProviderPair {
            provider: ConformanceProvider::Iroh.provider_id(),
            first_factory: Box::new(first.transport_factory),
            second_factory: Box::new(second.transport_factory),
            first_endpoint,
            second_endpoint,
            lifecycles: Some((first.lifecycle, second.lifecycle)),
            runtime: Some(runtime),
        }
    }

    fn current_timestamp() -> Timestamp {
        let millis =
            SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after epoch").as_millis();
        Timestamp::from_unix_millis(i64::try_from(millis).expect("timestamp fits i64"))
            .expect("valid timestamp")
    }

    fn drive_ready(
        first: &mut PeerLink<PersistedRelationships, SharedSigner>,
        second: &mut PeerLink<PersistedRelationships, SharedSigner>,
        first_contact_id: ContactId,
        second_contact_id: ContactId,
    ) {
        first.prime_contact(second_contact_id).expect("prime durable relationship");
        let deadline = Instant::now() + Duration::from_secs(5);
        while first.connection_state(second_contact_id) != PeerConnectionState::Ready
            || second.connection_state(first_contact_id) != PeerConnectionState::Ready
        {
            let _ = first
                .maintenance(&[second_contact_id], current_timestamp())
                .expect("maintain first peer");
            let _ = second
                .maintenance(&[first_contact_id], current_timestamp())
                .expect("maintain second peer");
            assert!(Instant::now() < deadline, "provider handshake timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn transfer_and_ack(
        sender: &mut PeerLink<PersistedRelationships, SharedSigner>,
        receiver: &mut PeerLink<PersistedRelationships, SharedSigner>,
        sender_contact_id: ContactId,
        receiver_contact_id: ContactId,
        envelope_id: OpaqueId,
        payload: &[u8],
    ) {
        sender
            .send_envelope(receiver_contact_id, envelope_id, TEXT_KIND, payload.to_vec())
            .expect("send conformance envelope");
        let deadline = Instant::now() + Duration::from_secs(3);
        let inbound = loop {
            let _ = receiver
                .maintenance(&[sender_contact_id], current_timestamp())
                .expect("maintain receiving peer");
            if let Some(inbound) = receiver.take_inbound() {
                break inbound;
            }
            assert!(Instant::now() < deadline, "conformance envelope timed out");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(inbound.contact_id, sender_contact_id);
        assert_eq!(inbound.envelope_id, envelope_id);
        assert_eq!(inbound.ciphertext, payload);
        receiver
            .send_ack(sender_contact_id, envelope_id, AckStatus::Accepted)
            .expect("send transport ACK");
        loop {
            let _ = sender
                .maintenance(&[receiver_contact_id], current_timestamp())
                .expect("maintain ACK sender");
            if let Some(receipt) = sender
                .poll_envelope_ack(receiver_contact_id, envelope_id)
                .expect("poll transport ACK")
            {
                assert_eq!(receipt, LinkAck::Accepted);
                break;
            }
            assert!(Instant::now() < deadline, "transport ACK timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn run_pair_persist_handshake_message_ack(mut pair: ProviderPair) {
        let (first_node, second_node) =
            complete_pairing(&pair.provider, &pair.first_endpoint, &pair.second_endpoint);
        let first_contact_id = second_node.contact_id;
        let second_contact_id = first_node.contact_id;
        let mut first = PeerLink::with_transport_factory(
            pair.first_factory,
            first_node.relationships,
            first_node.signer,
            first_node.public_identity.identity_id().to_opaque(),
        );
        let mut second = PeerLink::with_transport_factory(
            pair.second_factory,
            second_node.relationships,
            second_node.signer,
            second_node.public_identity.identity_id().to_opaque(),
        );

        drive_ready(&mut first, &mut second, first_contact_id, second_contact_id);
        transfer_and_ack(
            &mut first,
            &mut second,
            first_contact_id,
            second_contact_id,
            OpaqueId::from_u128(9_001),
            b"pair-persist-handshake-message",
        );
        first.shutdown();
        second.shutdown();
        if let Some((first, second)) = pair.lifecycles.as_mut() {
            first.shutdown();
            second.shutdown();
        }
        drop(pair.runtime);
    }

    #[test]
    fn memory_pair_persist_handshake_message_ack() {
        let network = MemoryNetwork::default();
        run_pair_persist_handshake_message_ack(memory_pair(&network, b"memory-a", b"memory-b"));
    }

    #[test]
    fn iroh_persisted_route_factory_handshake_message_ack() {
        run_pair_persist_handshake_message_ack(iroh_pair());
    }

    #[test]
    fn memory_restart_and_non_preferred_durable_dial_need_no_repairing() {
        let network = MemoryNetwork::default();
        let first_endpoint = b"restart-a".to_vec();
        let second_endpoint = b"restart-b".to_vec();
        let initial = memory_pair(&network, &first_endpoint, &second_endpoint);
        let (first_node, second_node) =
            complete_pairing(&initial.provider, &initial.first_endpoint, &initial.second_endpoint);
        drop(initial);
        let first_contact_id = second_node.contact_id;
        let second_contact_id = first_node.contact_id;

        for pass in 0_u128..2 {
            let pair = memory_pair(&network, &first_endpoint, &second_endpoint);
            let mut first = PeerLink::with_transport_factory(
                pair.first_factory,
                first_node.relationships.clone(),
                first_node.signer.clone(),
                first_node.public_identity.identity_id().to_opaque(),
            );
            let mut second = PeerLink::with_transport_factory(
                pair.second_factory,
                second_node.relationships.clone(),
                second_node.signer.clone(),
                second_node.public_identity.identity_id().to_opaque(),
            );
            if pass == 0 {
                drive_ready(&mut first, &mut second, first_contact_id, second_contact_id);
            } else {
                second.prime_contact(first_contact_id).expect("prime non-preferred durable sender");
                drive_ready(&mut first, &mut second, first_contact_id, second_contact_id);
                transfer_and_ack(
                    &mut second,
                    &mut first,
                    second_contact_id,
                    first_contact_id,
                    OpaqueId::from_u128(9_100 + pass),
                    b"durable message after restart",
                );
            }
            first.shutdown();
            second.shutdown();
        }
    }

    #[test]
    fn iroh_restart_uses_persisted_relationship_after_route_refresh() {
        let mut initial = iroh_pair();
        let (mut first_node, mut second_node) =
            complete_pairing(&initial.provider, &initial.first_endpoint, &initial.second_endpoint);
        let first_contact_id = second_node.contact_id;
        let second_contact_id = first_node.contact_id;
        if let Some((first, second)) = initial.lifecycles.as_mut() {
            first.shutdown();
            second.shutdown();
        }
        drop(initial);

        let mut restarted = iroh_pair();
        first_node.relationships.refresh_endpoint(
            second_contact_id,
            &restarted.provider,
            restarted.second_endpoint.clone(),
        );
        second_node.relationships.refresh_endpoint(
            first_contact_id,
            &restarted.provider,
            restarted.first_endpoint.clone(),
        );
        let mut first = PeerLink::with_transport_factory(
            restarted.first_factory,
            first_node.relationships,
            first_node.signer,
            first_node.public_identity.identity_id().to_opaque(),
        );
        let mut second = PeerLink::with_transport_factory(
            restarted.second_factory,
            second_node.relationships,
            second_node.signer,
            second_node.public_identity.identity_id().to_opaque(),
        );
        second.prime_contact(first_contact_id).expect("prime durable sender after Iroh restart");
        drive_ready(&mut first, &mut second, first_contact_id, second_contact_id);
        transfer_and_ack(
            &mut second,
            &mut first,
            second_contact_id,
            first_contact_id,
            OpaqueId::from_u128(9_202),
            b"persisted relationship after Iroh restart",
        );
        first.shutdown();
        second.shutdown();
        if let Some((first, second)) = restarted.lifecycles.as_mut() {
            first.shutdown();
            second.shutdown();
        }
    }

    #[test]
    fn iroh_stale_route_is_rejected_then_refresh_succeeds() {
        let mut pair = iroh_pair();
        let (first_node, _) =
            complete_pairing(&pair.provider, &pair.first_endpoint, &pair.second_endpoint);
        let contact = first_node
            .relationships
            .list()
            .expect("persisted contacts")
            .into_iter()
            .next()
            .expect("remote contact");
        let (first_lifecycle, second_lifecycle) =
            pair.lifecycles.as_mut().expect("Iroh lifecycles");
        first_lifecycle.network_changed(current_timestamp());
        assert!(matches!(
            pair.first_factory.connect(&contact),
            Err(TransportFactoryError::RouteStale)
        ));

        let deadline = Instant::now() + Duration::from_secs(5);
        while first_lifecycle.peer_endpoint_bytes().is_err() {
            assert!(Instant::now() < deadline, "Iroh route refresh timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut refreshed = pair.first_factory.connect(&contact).expect("refreshed route dials");
        refreshed.connect().expect("connect refreshed Iroh route");
        refreshed.close().expect("close refreshed route");
        first_lifecycle.shutdown();
        second_lifecycle.shutdown();
    }

    #[test]
    fn opaque_route_topology_remains_provider_neutral() {
        let memory = persisted_route(ConformanceProvider::Memory, TransportTopology::Direct);
        let iroh = persisted_route(ConformanceProvider::Iroh, TransportTopology::Direct);
        assert!(memory.persisted && iroh.persisted);
        assert_eq!(memory.path.topology, iroh.path.topology);
    }
}
