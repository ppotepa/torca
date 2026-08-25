use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use torca_client_engine::{ClientEngine, ClientEngineActor, EngineCommand};
use torca_crypto::RustPairingCrypto;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityId, Profile, ProfileName};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_pairing_coordinator::{
    LocalPairingContext, PairingApprovalError, PairingApprovalPort, PairingCoordinator,
    PairingCoordinatorError, PairingCredentialError, PairingDerivedSecret, PairingPeerSecretStore,
    PairingRuntime, PairingSessionDelivery, PairingSessionServicePort, PairingSideToken,
    PairingSlotCapability, PairingSlotId,
};
use torca_pairing_protocol::PairingEnvelope;
use torca_relay::RelayBroker;
use torca_relay_protocol::{
    RelayCode, RelayJoinTicket, RelayMessageId, RelayOperationId, RelayRequest, RelayResponse,
    RelaySequence, RelaySideToken as WireRelaySideToken,
    RelaySlotCapability as WireRelaySlotCapability, RelaySlotId as WireRelaySlotId,
};

#[derive(Clone)]
struct SharedRelay(Rc<RefCell<RelayBroker>>);

impl SharedRelay {
    fn call(&self, request: RelayRequest) -> Result<RelayResponse, PairingCoordinatorError> {
        self.0
            .borrow_mut()
            .handle(request, Timestamp::from_unix_millis(1_000).expect("time"))
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
        let relay_code =
            RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
        match self.call(RelayRequest::Open {
            operation_id: RelayOperationId(capability.0),
            code: relay_code,
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
        let relay_code =
            RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
        match self.call(RelayRequest::Join {
            operation_id: RelayOperationId(token.0),
            code: relay_code,
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
struct TestApproval;
impl PairingApprovalPort for TestApproval {
    fn transcript_digest(
        &self,
        creator: &PairingEnvelope,
        joiner: &PairingEnvelope,
    ) -> Result<[u8; 32], PairingApprovalError> {
        let mut digest = [0_u8; 32];
        for byte in creator
            .transcript_component()
            .map_err(|_| PairingApprovalError::InvalidTranscript)?
            .into_iter()
            .chain(
                joiner
                    .transcript_component()
                    .map_err(|_| PairingApprovalError::InvalidTranscript)?,
            )
        {
            let index = usize::from(byte) % digest.len();
            digest[index] = digest[index].wrapping_mul(31).wrapping_add(byte);
        }
        Ok(digest)
    }

    fn sign_approval(
        &self,
        _key_id: torca_identity::KeyId,
        _context_id: OpaqueId,
        digest: [u8; 32],
    ) -> Result<Vec<u8>, PairingApprovalError> {
        Ok(digest.to_vec())
    }

    fn verify_approval(
        &self,
        _remote: &torca_identity::PublicIdentity,
        _context_id: OpaqueId,
        digest: [u8; 32],
        proof: &[u8],
    ) -> Result<(), PairingApprovalError> {
        (proof == digest).then_some(()).ok_or(PairingApprovalError::InvalidProof)
    }
}

#[derive(Default)]
struct TestSecrets {
    next: u128,
    pairing_states: BTreeMap<PairingSessionId, Vec<u8>>,
}
impl PairingPeerSecretStore for TestSecrets {
    fn store_peer_secret(
        &mut self,
        _secret: PairingDerivedSecret,
    ) -> Result<OpaqueId, PairingCredentialError> {
        self.next += 1;
        Ok(OpaqueId::from_u128(self.next))
    }

    fn delete_peer_secret(&mut self, _handle: OpaqueId) -> Result<bool, PairingCredentialError> {
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

fn context(engine: &torca_client_engine::EngineHandle, name: &str) -> LocalPairingContext {
    let identity = engine.overview_snapshot().expect("snapshot").identity.expect("identity");
    LocalPairingContext {
        public_identity: identity.public().clone(),
        display_name: name.into(),
        capability_id: OpaqueId::from_u128(if name == "Alice" { 101 } else { 202 }),
        avatar: None,
        transport_provider: "tor".into(),
        transport_endpoint: format!("{}.onion", "a".repeat(56)).into_bytes(),
    }
}

#[test]
fn two_runtimes_commit_contacts_only_after_durable_acknowledgements() {
    let now = Timestamp::from_unix_millis(1_000).expect("time");
    let (creator_engine, creator_actor) = ClientEngineActor::spawn(ClientEngine::default());
    let (joiner_engine, joiner_actor) = ClientEngineActor::spawn(ClientEngine::default());
    for (engine, id, name) in [(&creator_engine, 1, "Alice"), (&joiner_engine, 2, "Bob")] {
        let _ = engine
            .dispatch(EngineCommand::CreateIdentity {
                identity_id: IdentityId::from_u128(id),
                profile: Some(Profile::new(ProfileName::new(name).expect("name"), None)),
                at: now,
            })
            .expect("identity");
    }
    let broker = Rc::new(RefCell::new(RelayBroker::default()));
    let creator_session_id = PairingSessionId::from_u128(77);
    let joiner_session_id = PairingSessionId::from_u128(88);
    let mut creator = PairingRuntime::new(
        PairingCoordinator::new(SharedRelay(broker.clone()), RustPairingCrypto::new()),
        creator_engine.clone(),
        TestApproval,
        TestSecrets::default(),
    );
    let mut joiner = PairingRuntime::new(
        PairingCoordinator::new(SharedRelay(broker.clone()), RustPairingCrypto::new()),
        joiner_engine.clone(),
        TestApproval,
        TestSecrets::default(),
    );

    // The relay invitation is available before either local onion service has
    // published. This is the cold-start path used by Android and desktop.
    let invitation = creator
        .create_invitation_pending_route(creator_session_id, now)
        .expect("create pending route");
    // Reconstructing the process runtime must retain the protected ephemeral transport state,
    // not strand an otherwise valid relay invitation after an Activity/service/process restart.
    let (_discarded_coordinator, _engine, _approval, creator_secrets) = creator.into_parts();
    let mut creator = PairingRuntime::new(
        PairingCoordinator::new(SharedRelay(broker.clone()), RustPairingCrypto::new()),
        creator_engine.clone(),
        TestApproval,
        creator_secrets,
    );
    assert_eq!(creator.restore_active_sessions().expect("restore creator"), 1);
    joiner
        .join_invitation_pending_route(
            joiner_session_id,
            invitation.code,
            Some(*invitation.ticket.as_bytes()),
        )
        .expect("join pending route");

    // Publishing either route later resumes the unchanged authenticated
    // offer/approval protocol; no contact exists before both routes arrive.
    assert!(
        creator
            .publish_local_offer(creator_session_id, context(&creator_engine, "Alice"))
            .expect("publish creator route")
    );
    assert!(
        joiner
            .publish_local_offer(joiner_session_id, context(&joiner_engine, "Bob"))
            .expect("publish joiner route")
    );
    let _ = creator.poll(creator_session_id, now).expect("creator offer exchange");
    let _ = joiner.poll(joiner_session_id, now).expect("joiner offer and approval");
    let _ = creator.poll(creator_session_id, now).expect("creator receives approval");
    creator.approve(creator_session_id, now).expect("creator approval");
    let _ = joiner.poll(joiner_session_id, now).expect("joiner commits and acknowledges");
    let _ = creator
        .poll(creator_session_id, now)
        .expect("creator commits and receives acknowledgement");
    let _ = joiner.poll(joiner_session_id, now).expect("joiner receives acknowledgement");

    for engine in [&creator_engine, &joiner_engine] {
        let snapshot = engine.overview_snapshot().expect("snapshot");
        assert_eq!(snapshot.contacts.len(), 1);
        assert_eq!(snapshot.conversations.len(), 1);
        assert!(snapshot.pairings.is_empty());
    }
    let _ = creator_actor.shutdown();
    let _ = joiner_actor.shutdown();
}
