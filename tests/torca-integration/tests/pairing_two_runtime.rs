use std::cell::RefCell;
use std::rc::Rc;

use torca_client_engine::{ClientEngine, ClientEngineActor, EngineCommand};
use torca_crypto::RustPairingCrypto;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityId, Profile, ProfileName};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_pairing_coordinator::{
    LocalPairingContext, PairingApprovalError, PairingApprovalPort, PairingCoordinator,
    PairingCoordinatorError, PairingCredentialError, PairingDerivedSecret, PairingPeerSecretStore,
    PairingRendezvousPort, PairingRuntime, PairingSideToken, PairingSlotCapability, PairingSlotId,
};
use torca_pairing_protocol::PairingEnvelope;
use torca_relay::RelayBroker;
use torca_relay_protocol::{
    RelayCode, RelayRequest, RelayResponse, RelaySideToken as WireRelaySideToken,
    RelaySlotCapability as WireRelaySlotCapability, RelaySlotId as WireRelaySlotId,
};

#[derive(Clone)]
struct SharedRelay(Rc<RefCell<RelayBroker>>);

impl SharedRelay {
    fn call(&self, request: RelayRequest) -> Result<RelayResponse, PairingCoordinatorError> {
        self.0
            .borrow_mut()
            .handle(request, Timestamp::from_unix_millis(1_000).expect("time"))
            .map_err(|_| PairingCoordinatorError::Rendezvous)
    }
}

impl PairingRendezvousPort for SharedRelay {
    fn open(
        &mut self,
        code: &PairingCode,
        expires_at: Timestamp,
        creator_blob: Vec<u8>,
        capability: PairingSlotCapability,
        token: PairingSideToken,
    ) -> Result<(PairingSlotId, Timestamp), PairingCoordinatorError> {
        let relay_code =
            RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
        match self.call(RelayRequest::Open {
            code: relay_code,
            expires_at,
            creator_blob,
            slot_capability: WireRelaySlotCapability(capability.0),
            creator_token: WireRelaySideToken(token.0),
        })? {
            RelayResponse::Opened { slot_id, expires_at } => {
                Ok((PairingSlotId(slot_id.0), expires_at))
            }
            _ => Err(PairingCoordinatorError::Rendezvous),
        }
    }

    fn join(
        &mut self,
        code: &PairingCode,
        joiner_blob: Vec<u8>,
        token: PairingSideToken,
    ) -> Result<(PairingSlotId, Timestamp, Vec<u8>), PairingCoordinatorError> {
        let relay_code =
            RelayCode::new(code.as_str()).map_err(|_| PairingCoordinatorError::Protocol)?;
        match self.call(RelayRequest::Join {
            code: relay_code,
            joiner_blob,
            joiner_token: WireRelaySideToken(token.0),
        })? {
            RelayResponse::Joined { slot_id, expires_at, creator_blob } => {
                Ok((PairingSlotId(slot_id.0), expires_at, creator_blob))
            }
            _ => Err(PairingCoordinatorError::Rendezvous),
        }
    }

    fn push(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
        blob: Vec<u8>,
    ) -> Result<(), PairingCoordinatorError> {
        match self.call(RelayRequest::Push {
            slot_id: WireRelaySlotId(slot.0),
            token: WireRelaySideToken(token.0),
            blob,
        })? {
            RelayResponse::Accepted => Ok(()),
            _ => Err(PairingCoordinatorError::Rendezvous),
        }
    }

    fn poll(
        &mut self,
        slot: PairingSlotId,
        token: PairingSideToken,
    ) -> Result<Vec<Vec<u8>>, PairingCoordinatorError> {
        match self.call(RelayRequest::Poll {
            slot_id: WireRelaySlotId(slot.0),
            token: WireRelaySideToken(token.0),
        })? {
            RelayResponse::Blobs(blobs) => Ok(blobs),
            _ => Err(PairingCoordinatorError::Rendezvous),
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
            _ => Err(PairingCoordinatorError::Rendezvous),
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
        _session_id: PairingSessionId,
        digest: [u8; 32],
    ) -> Result<Vec<u8>, PairingApprovalError> {
        Ok(digest.to_vec())
    }

    fn verify_approval(
        &self,
        _remote: &torca_identity::PublicIdentity,
        _session_id: PairingSessionId,
        digest: [u8; 32],
        proof: &[u8],
    ) -> Result<(), PairingApprovalError> {
        (proof == digest).then_some(()).ok_or(PairingApprovalError::InvalidProof)
    }
}

#[derive(Default)]
struct TestSecrets(u128);
impl PairingPeerSecretStore for TestSecrets {
    fn store_peer_secret(
        &mut self,
        _secret: PairingDerivedSecret,
    ) -> Result<OpaqueId, PairingCredentialError> {
        self.0 += 1;
        Ok(OpaqueId::from_u128(self.0))
    }

    fn delete_peer_secret(&mut self, _handle: OpaqueId) -> Result<bool, PairingCredentialError> {
        Ok(true)
    }
}

fn context(engine: &torca_client_engine::EngineHandle, name: &str) -> LocalPairingContext {
    let identity = engine.overview_snapshot().expect("snapshot").identity.expect("identity");
    LocalPairingContext {
        public_identity: identity.public().clone(),
        display_name: name.into(),
        onion_address: format!("{}.onion", "a".repeat(56)),
        capability_id: OpaqueId::from_u128(if name == "Alice" { 101 } else { 202 }),
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
    let session_id = PairingSessionId::from_u128(77);
    let mut creator = PairingRuntime::new(
        PairingCoordinator::new(SharedRelay(broker.clone()), RustPairingCrypto::new()),
        creator_engine.clone(),
        TestApproval,
        TestSecrets::default(),
    );
    let mut joiner = PairingRuntime::new(
        PairingCoordinator::new(SharedRelay(broker), RustPairingCrypto::new()),
        joiner_engine.clone(),
        TestApproval,
        TestSecrets::default(),
    );

    let invitation = creator
        .create_invitation(session_id, context(&creator_engine, "Alice"), now)
        .expect("create");
    joiner
        .join_invitation(session_id, invitation.code, context(&joiner_engine, "Bob"), now)
        .expect("join");
    let _ = creator.poll(session_id, now).expect("creator offer exchange");
    let _ = joiner.poll(session_id, now).expect("joiner offer and approval");
    let _ = creator.poll(session_id, now).expect("creator receives approval");
    creator.approve(session_id, now).expect("creator approval");
    let _ = joiner.poll(session_id, now).expect("joiner commits and acknowledges");
    let _ = creator.poll(session_id, now).expect("creator commits and receives acknowledgement");
    let _ = joiner.poll(session_id, now).expect("joiner receives acknowledgement");

    for engine in [&creator_engine, &joiner_engine] {
        let snapshot = engine.overview_snapshot().expect("snapshot");
        assert_eq!(snapshot.contacts.len(), 1);
        assert_eq!(snapshot.conversations.len(), 1);
        assert!(snapshot.pairings.is_empty());
    }
    let _ = creator_actor.shutdown();
    let _ = joiner_actor.shutdown();
}
