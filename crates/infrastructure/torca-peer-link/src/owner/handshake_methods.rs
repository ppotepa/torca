impl<S, K> PeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
fn accept_pending(&mut self, report: &mut PeerLinkReport) -> Result<(), PeerLinkError> {
    while self.pending.len() < MAX_PENDING_INCOMING {
        match self.listener.try_accept_transport().map_err(map_tor)? {
            Some(transport) => {
                self.pending.push(transport);
                report.accepted += 1;
            }
            None => break,
        }
    }
    if self.pending.len() >= MAX_PENDING_INCOMING
        && self
            .listener
            .try_accept_transport()
            .map_err(map_tor)?
            .is_some()
    {
        report.rejected += 1;
    }
    Ok(())
}

fn authenticate_pending(
    &mut self,
    now: Timestamp,
    report: &mut PeerLinkReport,
) -> Result<(), PeerLinkError> {
    let mut index = 0;
    while index < self.pending.len() {
        match self.try_authenticate(index, now) {
            Ok(AuthOutcome::Waiting) => index += 1,
            Ok(AuthOutcome::Authenticated(contact_id)) => {
                self.reconnect.remove(&contact_id);
                report.authenticated += 1;
            }
            Err(_) => {
                let mut rejected = self.pending.swap_remove(index);
                let _ = rejected.close();
                report.rejected += 1;
            }
        }
    }
    Ok(())
}

fn try_authenticate(
    &mut self,
    index: usize,
    now: Timestamp,
) -> Result<AuthOutcome, PeerLinkError> {
    let payload = match self.pending[index].try_receive() {
        Ok(Some(payload)) => payload,
        Ok(None) => return Ok(AuthOutcome::Waiting),
        Err(_) => return Err(PeerLinkError::Protocol),
    };
    let hello = match PeerCodec::decode(&payload).map_err(|_| PeerLinkError::Protocol)? {
        PeerMessage::Hello(hello) => hello,
        _ => return Err(PeerLinkError::Protocol),
    };
    let contact = self.contact_for_identity(hello.identity_id)?;
    let credential = self
        .relationships
        .credential_for_contact(contact.id())
        .map_err(map_contact)?
        .ok_or(PeerLinkError::Unauthorized)?;
    if hello.capability_id != credential.local_capability_id() {
        return Err(PeerLinkError::Unauthorized);
    }
    if self.incoming.contains_key(&contact.id()) {
        return Err(PeerLinkError::DuplicateConnection);
    }
    if self.outgoing.contains_key(&contact.id()) {
        if self.prefer_outgoing(&contact) {
            return Err(PeerLinkError::DuplicateConnection);
        }
        if let Some(mut outgoing) = self.outgoing.remove(&contact.id()) {
            let _ = outgoing.close();
        }
    }

    let verifier = verifier_for(&contact)?;
    let policy = HandshakePolicy {
        expected_identity: contact.remote_identity().identity_id().to_opaque(),
        expected_capability: credential.local_capability_id(),
        max_clock_skew_ms: MAX_CLOCK_SKEW_MS,
    };
    let transport = self.pending.swap_remove(index);
    let mut session = PeerSession::new(transport, verifier, policy);
    session.receive(&payload, now).map_err(map_session)?;
    let ack = HandshakeAck::signed(hello.session_id, hello.nonce, &self.signer)
        .map_err(|_| PeerLinkError::Protocol)?;
    session.send_handshake_ack(ack).map_err(map_session)?;
    let contact_id = contact.id();
    self.incoming.insert(contact_id, session);
    self.observe(
        contact_id,
        Some(TransportDirection::Rx),
        TransportOperation::Handshake,
        OperationPhase::Completed,
        None,
        now,
    );
    Ok(AuthOutcome::Authenticated(contact_id))
}

fn connect_outgoing(
    &mut self,
    contact_id: ContactId,
    now: Timestamp,
) -> Result<(), PeerLinkError> {
    if self.outgoing.contains_key(&contact_id) || self.incoming.contains_key(&contact_id) {
        return Err(PeerLinkError::DuplicateConnection);
    }
    let contact = self
        .relationships
        .get(contact_id)
        .map_err(map_contact)?
        .ok_or(PeerLinkError::ContactNotFound)?;
    let verifier = verifier_for(&contact)?;
    let policy = HandshakePolicy {
        expected_identity: contact.remote_identity().identity_id().to_opaque(),
        expected_capability: contact.route().capability_id(),
        max_clock_skew_ms: MAX_CLOCK_SKEW_MS,
    };
    let transport = TorPeerTransport::new(
        self.tor_client.clone(),
        contact.route().onion_address(),
        TOR_PEER_VIRTUAL_PORT,
    );
    let mut session = PeerSession::new(transport, verifier, policy);
    let session_id = self.random_id()?;
    let nonce = self.random_32()?;
    let hello = HandshakeHello::signed(
        session_id,
        self.local_identity_id,
        contact.route().capability_id(),
        now,
        nonce,
        &self.signer,
    )
    .map_err(|_| PeerLinkError::Protocol)?;
    self.observe(
        contact_id,
        None,
        TransportOperation::Connect,
        OperationPhase::Started,
        None,
        now,
    );
    if let Err(error) = session.connect(hello).map_err(map_session) {
        self.observe(
            contact_id,
            None,
            TransportOperation::Connect,
            OperationPhase::Failed,
            None,
            now,
        );
        return Err(error);
    }
    self.outgoing.insert(contact_id, session);
    Ok(())
}
}
