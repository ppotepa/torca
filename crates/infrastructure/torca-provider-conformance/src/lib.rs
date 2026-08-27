//! Cross-provider peer contract. This crate intentionally contains test-only
//! composition code and no production provider selection.

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use tokio::runtime::Runtime;
    use torca_client_engine::{
        AvatarGenomeRecord, ClientEngine, ClientEngineActor, EngineCommand, EngineError,
        InMemoryRelationshipRepository, RelationshipRepository,
    };
    use torca_contacts::{
        Contact, ContactError, ContactId, ContactRepository, ContactRoute, PeerCredential,
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
        IdentityId, IdentityKey, IdentityKeyProvider, InMemoryIdentityRepository, KeyId, Profile,
        ProfileName, PublicIdentity,
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
    use torca_relay::RelayBroker;
    use torca_relay_protocol::{
        RelayCode, RelayJoinTicket, RelayMessageId, RelayOperationId, RelayRequest, RelayResponse,
        RelaySequence, RelaySideToken as WireRelaySideToken,
        RelaySlotCapability as WireRelaySlotCapability, RelaySlotId as WireRelaySlotId,
    };
    use torca_transport_api::{PeerTransportFactory, TransportKind};
    use torca_transport_iroh::{IrohComposition, IrohEndpointProfile};
    use torca_transport_memory::MemoryNetwork;

    const TEXT_KIND: u16 = 1;
    const ATTACHMENT_KIND: u16 = 3;
    const CONTROL_KIND: u16 = 5;

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

    #[derive(Clone)]
    struct SharedRelay(Rc<RefCell<RelayBroker>>);

    impl SharedRelay {
        fn call(&self, request: RelayRequest) -> Result<RelayResponse, PairingCoordinatorError> {
            self.0
                .borrow_mut()
                .handle(request, Timestamp::from_unix_millis(1_000).expect("relay time"))
                .map_err(|_| PairingCoordinatorError::SessionService)
        }
    }

    impl PairingSessionServicePort for SharedRelay {
        fn open(
            &mut self,
            code: &PairingCode,
            expires_at: Timestamp,
            creator_blob: Vec<u8>,
            capability: PairingSlotCapability,
            token: PairingSideToken,
            ticket: [u8; 16],
        ) -> Result<(PairingSlotId, Timestamp), PairingCoordinatorError> {
            let code =
                RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
            match self.call(RelayRequest::Open {
                operation_id: RelayOperationId(capability.0),
                code,
                expires_at,
                creator_blob,
                slot_capability: WireRelaySlotCapability(capability.0),
                creator_token: WireRelaySideToken(token.0),
                ticket: RelayJoinTicket(ticket),
            })? {
                RelayResponse::Opened { slot_id, expires_at } => {
                    Ok((PairingSlotId(slot_id.0), expires_at))
                }
                _ => Err(PairingCoordinatorError::SessionService),
            }
        }

        fn join(
            &mut self,
            code: &PairingCode,
            joiner_blob: Vec<u8>,
            token: PairingSideToken,
            ticket: Option<[u8; 16]>,
            _bootstrap: Option<&torca_pairing_protocol::PairingBootstrapDescriptor>,
        ) -> Result<(PairingSlotId, Timestamp, Vec<u8>), PairingCoordinatorError> {
            let code =
                RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
            match self.call(RelayRequest::Join {
                operation_id: RelayOperationId(token.0),
                code,
                joiner_blob,
                joiner_token: WireRelaySideToken(token.0),
                ticket: ticket.map(RelayJoinTicket),
            })? {
                RelayResponse::Joined { slot_id, expires_at, creator_blob } => {
                    Ok((PairingSlotId(slot_id.0), expires_at, creator_blob))
                }
                _ => Err(PairingCoordinatorError::SessionService),
            }
        }

        fn push(
            &mut self,
            message_id: OpaqueId,
            slot: PairingSlotId,
            token: PairingSideToken,
            blob: Vec<u8>,
        ) -> Result<(), PairingCoordinatorError> {
            match self.call(RelayRequest::Push {
                operation_id: RelayOperationId(message_id),
                message_id: RelayMessageId(message_id),
                slot_id: WireRelaySlotId(slot.0),
                token: WireRelaySideToken(token.0),
                blob,
            })? {
                RelayResponse::Accepted => Ok(()),
                _ => Err(PairingCoordinatorError::SessionService),
            }
        }

        fn poll(
            &mut self,
            slot: PairingSlotId,
            token: PairingSideToken,
            after: u64,
        ) -> Result<Vec<PairingSessionDelivery>, PairingCoordinatorError> {
            match self.call(RelayRequest::Poll {
                slot_id: WireRelaySlotId(slot.0),
                token: WireRelaySideToken(token.0),
                after: RelaySequence(after),
            })? {
                RelayResponse::Deliveries(deliveries) => Ok(deliveries
                    .into_iter()
                    .map(|delivery| PairingSessionDelivery {
                        sequence: delivery.sequence.0,
                        blob: delivery.blob,
                    })
                    .collect()),
                _ => Err(PairingCoordinatorError::SessionService),
            }
        }

        fn ack(
            &mut self,
            slot: PairingSlotId,
            token: PairingSideToken,
            up_to: u64,
        ) -> Result<(), PairingCoordinatorError> {
            match self.call(RelayRequest::Ack {
                slot_id: WireRelaySlotId(slot.0),
                token: WireRelaySideToken(token.0),
                up_to: RelaySequence(up_to),
            })? {
                RelayResponse::Acked(_) => Ok(()),
                _ => Err(PairingCoordinatorError::SessionService),
            }
        }

        fn close(
            &mut self,
            slot: PairingSlotId,
            capability: PairingSlotCapability,
        ) -> Result<(), PairingCoordinatorError> {
            match self.call(RelayRequest::Close {
                slot_id: WireRelaySlotId(slot.0),
                capability: WireRelaySlotCapability(capability.0),
            })? {
                RelayResponse::Closed => Ok(()),
                _ => Err(PairingCoordinatorError::SessionService),
            }
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

    #[derive(Clone, Default)]
    struct PersistedRelationships {
        contacts: BTreeMap<ContactId, Contact>,
        credentials: BTreeMap<ContactId, PeerCredential>,
    }

    impl PersistedRelationships {
        fn from_pairing_completion(contact: Contact, credential: PeerCredential) -> Self {
            Self {
                contacts: BTreeMap::from([(contact.id(), contact)]),
                credentials: BTreeMap::from([(credential.contact_id(), credential)]),
            }
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

    fn identity(identity_id: IdentityId) -> (PublicIdentity, SharedSigner) {
        let mut keys = ManagedIdentityKeys::new(RustCryptoProvider, SharedSecretStore::default());
        let generated = keys.generate_signing_key().expect("generate identity signing key");
        let public_key =
            IdentityKey::new(generated.key_id, generated.algorithm, generated.public_key)
                .expect("valid public identity key");
        let signer = SharedSigner { keys: Arc::new(keys), key_id: generated.key_id };
        (PublicIdentity::new(identity_id, public_key, 0), signer)
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
        kind: TransportKind,
        first_endpoint: &[u8],
        second_endpoint: &[u8],
    ) -> (PersistedPairingNode, PersistedPairingNode) {
        let now = Timestamp::from_unix_millis(1_000).expect("pairing time");
        let first = pairing_node(IdentityId::from_u128(101), "Alice", now);
        let second = pairing_node(IdentityId::from_u128(202), "Bob", now);
        let broker = Rc::new(RefCell::new(RelayBroker::default()));
        let first_session_id = PairingSessionId::from_u128(77);
        let second_session_id = PairingSessionId::from_u128(88);
        let mut first_pairing = PairingRuntime::new(
            PairingCoordinator::new(SharedRelay(Rc::clone(&broker)), RustPairingCrypto::new()),
            first.engine.clone(),
            ManagedIdentityKeys::new(RustCryptoProvider, first.secret_store.clone()),
            PairingSecrets::default(),
        );
        let mut second_pairing = PairingRuntime::new(
            PairingCoordinator::new(SharedRelay(Rc::clone(&broker)), RustPairingCrypto::new()),
            second.engine.clone(),
            ManagedIdentityKeys::new(RustCryptoProvider, second.secret_store.clone()),
            PairingSecrets::default(),
        );

        let invitation = first_pairing
            .create_invitation_pending_route(first_session_id, now)
            .expect("create provider conformance invitation");
        second_pairing
            .join_invitation_pending_route(
                second_session_id,
                invitation.code,
                Some(*invitation.ticket.as_bytes()),
            )
            .expect("join provider conformance invitation");
        first_pairing
            .publish_local_offer(
                first_session_id,
                LocalPairingContext {
                    public_identity: first.public_identity.clone(),
                    display_name: "Alice".into(),
                    capability_id: OpaqueId::from_u128(1_001),
                    avatar: None,
                    transport_provider: kind.wire_value().into(),
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
                    capability_id: OpaqueId::from_u128(2_002),
                    avatar: None,
                    transport_provider: kind.wire_value().into(),
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
            first_report.completed_contact.expect("first completed contact report").contact_id;
        let second_contact_id =
            second_report.completed_contact.expect("second completed contact report").contact_id;
        assert_eq!(first_contact_id, second_contact_id);
        for engine in [&first.engine, &second.engine] {
            let snapshot = engine.overview_snapshot().expect("post-pairing snapshot");
            assert_eq!(snapshot.contacts.len(), 1);
            assert_eq!(snapshot.conversations.len(), 1);
            assert!(snapshot.pairings.is_empty());
        }

        // The bootstrap lane is gone before PeerLink receives the persisted
        // contacts, credentials and provider factories.
        drop(first_pairing);
        drop(second_pairing);
        drop(broker);
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
        kind: TransportKind,
        first_factory: Box<dyn PeerTransportFactory>,
        second_factory: Box<dyn PeerTransportFactory>,
        first_endpoint: Vec<u8>,
        second_endpoint: Vec<u8>,
        _runtime: Option<Arc<Runtime>>,
    }

    fn memory_pair() -> ProviderPair {
        let network = MemoryNetwork::default();
        let first = network.bind(b"memory-node-a".to_vec()).expect("bind first memory node");
        let second = network.bind(b"memory-node-b".to_vec()).expect("bind second memory node");
        ProviderPair {
            kind: TransportKind::Memory,
            first_endpoint: first.endpoint().to_vec(),
            second_endpoint: second.endpoint().to_vec(),
            first_factory: Box::new(first),
            second_factory: Box::new(second),
            _runtime: None,
        }
    }

    fn iroh_direct_pair() -> ProviderPair {
        let runtime = Arc::new(Runtime::new().expect("Iroh Tokio runtime"));
        let mut first_store = InMemoryProtectedSecretStore::default();
        let mut second_store = InMemoryProtectedSecretStore::default();
        let first = IrohComposition::bind_with_profile(
            Arc::clone(&runtime),
            &mut first_store,
            IrohEndpointProfile::DirectOnly,
            false,
        )
        .expect("bind first direct Iroh node");
        let second = IrohComposition::bind_with_profile(
            Arc::clone(&runtime),
            &mut second_store,
            IrohEndpointProfile::DirectOnly,
            false,
        )
        .expect("bind second direct Iroh node");
        ProviderPair {
            kind: TransportKind::Iroh,
            first_endpoint: first.peer_endpoint_bytes().expect("first Iroh route"),
            second_endpoint: second.peer_endpoint_bytes().expect("second Iroh route"),
            first_factory: Box::new(first.transport_factory),
            second_factory: Box::new(second.transport_factory),
            _runtime: Some(runtime),
        }
    }

    fn timestamp() -> Timestamp {
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
            let _ =
                first.maintenance(&[second_contact_id], timestamp()).expect("maintain first peer");
            let _ =
                second.maintenance(&[first_contact_id], timestamp()).expect("maintain second peer");
            assert!(Instant::now() < deadline, "provider handshake timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn transfer_and_receipt(
        sender: &mut PeerLink<PersistedRelationships, SharedSigner>,
        receiver: &mut PeerLink<PersistedRelationships, SharedSigner>,
        sender_contact_id: ContactId,
        receiver_contact_id: ContactId,
        envelope_id: OpaqueId,
        message_kind: u16,
        payload: &[u8],
    ) {
        sender
            .send_envelope(receiver_contact_id, envelope_id, message_kind, payload.to_vec())
            .expect("send conformance envelope");
        let deadline = Instant::now() + Duration::from_secs(3);
        let inbound = loop {
            let _ = receiver
                .maintenance(&[sender_contact_id], timestamp())
                .expect("maintain receiving peer");
            if let Some(inbound) = receiver.take_inbound() {
                break inbound;
            }
            assert!(Instant::now() < deadline, "conformance envelope timed out");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(inbound.contact_id, sender_contact_id);
        assert_eq!(inbound.envelope_id, envelope_id);
        assert_eq!(inbound.message_kind, message_kind);
        assert_eq!(inbound.ciphertext, payload);
        receiver
            .send_ack(sender_contact_id, envelope_id, AckStatus::Accepted)
            .expect("send transport receipt");
        loop {
            let _ = sender
                .maintenance(&[receiver_contact_id], timestamp())
                .expect("maintain receipt sender");
            if let Some(receipt) = sender
                .poll_envelope_ack(receiver_contact_id, envelope_id)
                .expect("poll transport receipt")
            {
                assert_eq!(receipt, LinkAck::Accepted);
                break;
            }
            assert!(Instant::now() < deadline, "transport receipt timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn run_peer_conformance(pair: ProviderPair) {
        let (first_node, second_node) =
            complete_pairing(pair.kind, &pair.first_endpoint, &pair.second_endpoint);
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
        transfer_and_receipt(
            &mut first,
            &mut second,
            first_contact_id,
            second_contact_id,
            OpaqueId::from_u128(9_001),
            TEXT_KIND,
            b"text a-to-b",
        );
        transfer_and_receipt(
            &mut second,
            &mut first,
            second_contact_id,
            first_contact_id,
            OpaqueId::from_u128(9_002),
            TEXT_KIND,
            b"text b-to-a",
        );
        transfer_and_receipt(
            &mut first,
            &mut second,
            first_contact_id,
            second_contact_id,
            OpaqueId::from_u128(9_003),
            ATTACHMENT_KIND,
            b"attachment chunk",
        );
        transfer_and_receipt(
            &mut second,
            &mut first,
            second_contact_id,
            first_contact_id,
            OpaqueId::from_u128(9_004),
            CONTROL_KIND,
            b"control frame",
        );
        first.shutdown();
        second.shutdown();
    }

    #[test]
    fn memory_provider_satisfies_peer_contract() {
        run_peer_conformance(memory_pair());
    }

    #[test]
    fn iroh_direct_provider_satisfies_peer_contract() {
        run_peer_conformance(iroh_direct_pair());
    }

    #[test]
    fn restart_and_non_preferred_durable_sender_need_no_new_pairing() {
        let network = MemoryNetwork::default();
        let first_endpoint = b"restart-node-a".to_vec();
        let second_endpoint = b"restart-node-b".to_vec();
        let (first_identity, first_signer) = identity(IdentityId::from_u128(101));
        let (second_identity, second_signer) = identity(IdentityId::from_u128(202));
        let first_contact_id = ContactId::from_u128(31);
        let second_contact_id = ContactId::from_u128(32);
        let first_capability = OpaqueId::from_u128(3_101);
        let second_capability = OpaqueId::from_u128(3_202);
        let first_contact = Contact::new(
            first_contact_id,
            first_identity.clone(),
            ContactRoute::for_provider_endpoint(first_capability, "memory", first_endpoint.clone())
                .expect("first restart route"),
            Timestamp::UNIX_EPOCH,
        );
        let second_contact = Contact::new(
            second_contact_id,
            second_identity.clone(),
            ContactRoute::for_provider_endpoint(
                second_capability,
                "memory",
                second_endpoint.clone(),
            )
            .expect("second restart route"),
            Timestamp::UNIX_EPOCH,
        );
        let first_relationships = PersistedRelationships::from_pairing_completion(
            second_contact,
            PeerCredential::new(second_contact_id, first_capability, OpaqueId::from_u128(3_303))
                .expect("first restart credential"),
        );
        let second_relationships = PersistedRelationships::from_pairing_completion(
            first_contact,
            PeerCredential::new(first_contact_id, second_capability, OpaqueId::from_u128(3_404))
                .expect("second restart credential"),
        );

        for pass in 0_u128..2 {
            let first_factory =
                network.bind(first_endpoint.clone()).expect("rebind persisted first endpoint");
            let second_factory =
                network.bind(second_endpoint.clone()).expect("rebind persisted second endpoint");
            let mut first = PeerLink::with_transport_factory(
                Box::new(first_factory),
                first_relationships.clone(),
                first_signer.clone(),
                first_identity.identity_id().to_opaque(),
            );
            let mut second = PeerLink::with_transport_factory(
                Box::new(second_factory),
                second_relationships.clone(),
                second_signer.clone(),
                second_identity.identity_id().to_opaque(),
            );

            if pass == 0 {
                drive_ready(&mut first, &mut second, first_contact_id, second_contact_id);
            } else {
                // The second identity is the non-preferred dialer. Durable
                // outbox demand must still initiate a fresh post-restart link.
                second.prime_contact(first_contact_id).expect("prime non-preferred durable sender");
                let deadline = Instant::now() + Duration::from_secs(3);
                while first.connection_state(second_contact_id) != PeerConnectionState::Ready
                    || second.connection_state(first_contact_id) != PeerConnectionState::Ready
                {
                    let _ = first
                        .maintenance(&[second_contact_id], timestamp())
                        .expect("maintain restarted first peer");
                    let _ = second
                        .maintenance(&[first_contact_id], timestamp())
                        .expect("maintain restarted second peer");
                    assert!(Instant::now() < deadline, "restart handshake timed out");
                }
                transfer_and_receipt(
                    &mut second,
                    &mut first,
                    second_contact_id,
                    first_contact_id,
                    OpaqueId::from_u128(9_100 + pass),
                    TEXT_KIND,
                    b"durable message after restart",
                );
            }
            first.shutdown();
            second.shutdown();
        }
    }

    #[test]
    fn stale_memory_route_blocks_dial_until_provider_refresh() {
        let network = MemoryNetwork::default();
        let mut first = network.bind(b"route-node-a".to_vec()).expect("bind first route");
        let mut second = network.bind(b"route-node-b".to_vec()).expect("bind second route");
        let (remote_identity, _) = identity(IdentityId::from_u128(88));
        let contact = Contact::new(
            ContactId::from_u128(89),
            remote_identity,
            ContactRoute::for_provider_endpoint(
                OpaqueId::from_u128(90),
                "memory",
                second.endpoint().to_vec(),
            )
            .expect("valid route"),
            Timestamp::UNIX_EPOCH,
        );

        first.mark_route_stale();
        assert!(matches!(
            first.connect(&contact),
            Err(torca_transport_api::TransportFactoryError::RouteStale)
        ));
        first.mark_route_refreshed();
        let mut outgoing = first.connect(&contact).expect("refreshed route can dial");
        outgoing.connect().expect("connect refreshed transport");
        assert!(second.accept().expect("accept refreshed route").is_some());
    }
}
