impl<S, K> PeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    fn accept_pending(&mut self, report: &mut PeerLinkReport) -> Result<(), PeerLinkError> {
        while self.pending.len() < MAX_PENDING_INCOMING {
            match self.transport_factory.accept().map_err(map_transport_factory)? {
                Some(mut transport) => {
                    if let Some(waker) = &self.waker {
                        transport.set_waker(Arc::clone(waker));
                    }
                    self.pending.push(transport);
                    report.accepted += 1;
                }
                None => break,
            }
        }
        if self.pending.len() >= MAX_PENDING_INCOMING
            && self.transport_factory.accept().map_err(map_transport_factory)?.is_some()
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
        if let Some(waker) = &self.waker {
            session.set_waker(Arc::clone(waker));
        }
        session.receive(&payload, now).map_err(map_session)?;
        let ack = HandshakeAck::signed(hello.session_id, hello.nonce, &self.signer)
            .map_err(|_| PeerLinkError::Protocol)?;
        session.send_handshake_ack(ack).map_err(map_session)?;
        let contact_id = contact.id();
        // A fresh authenticated session must receive the route even if the
        // same generation was sent on an older session that the peer may not
        // have persisted before restarting.
        self.advertised_route_generations.remove(&contact_id);
        self.advertise_route(contact_id, &mut session)?;
        self.incoming.insert(contact_id, session);
        self.observe_with_stage(TelemetryEvent {
            contact_id,
            direction: Some(TransportDirection::Rx),
            operation: TransportOperation::Handshake,
            phase: OperationPhase::Completed,
            correlation_id: None,
            at: now,
            stage: Some(TransportStage::Handshake),
            error_code: None,
        });
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
        // Providers such as Iroh migrate their local endpoint asynchronously
        // after a network-generation event. Treat that short interval as a
        // retryable NotReady state before touching the contact route; this
        // keeps the generic peer link from creating a dial against a stale
        // local address and avoids a reconnect storm.
        let route_state = self.provider_routing.as_ref().map_or_else(
            || torca_transport_api::ProviderRouteState::Fresh,
            |routing| routing.route_state(),
        );
        if route_state == torca_transport_api::ProviderRouteState::Stale {
            self.observe_with_stage(TelemetryEvent {
                contact_id,
                direction: None,
                operation: TransportOperation::Connect,
                phase: OperationPhase::Failed,
                correlation_id: None,
                at: now,
                stage: Some(TransportStage::RouteStale),
                error_code: Some(torca_foundation::ErrorCode::new("peer.route_stale")),
            });
            return Err(PeerLinkError::NotReady);
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
        let transport = match self.transport_factory.connect(&contact) {
            Ok(transport) => transport,
            Err(error) => {
                let stage = if error == torca_transport_api::TransportFactoryError::RouteStale {
                    TransportStage::RouteStale
                } else {
                    TransportStage::Factory
                };
                self.observe_with_stage(TelemetryEvent {
                    contact_id,
                    direction: None,
                    operation: TransportOperation::Connect,
                    phase: OperationPhase::Failed,
                    correlation_id: None,
                    at: now,
                    stage: Some(stage),
                    error_code: Some(torca_foundation::ErrorCode::new("peer.factory_failed")),
                });
                return Err(map_transport_factory(error));
            }
        };
        let mut session = PeerSession::new(transport, verifier, policy);
        if let Some(waker) = &self.waker {
            session.set_waker(Arc::clone(waker));
        }
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
        self.observe_with_stage(TelemetryEvent {
            contact_id,
            direction: None,
            operation: TransportOperation::Connect,
            phase: OperationPhase::Started,
            correlation_id: None,
            at: now,
            stage: Some(TransportStage::Connect),
            error_code: None,
        });
        if let Err(error) = session.connect(hello).map_err(map_session) {
            self.observe_with_stage(TelemetryEvent {
                contact_id,
                direction: None,
                operation: TransportOperation::Connect,
                phase: OperationPhase::Failed,
                correlation_id: None,
                at: now,
                stage: Some(TransportStage::Connect),
                error_code: Some(torca_foundation::ErrorCode::new("peer.connect_failed")),
            });
            return Err(error);
        }
        self.observe_with_stage(TelemetryEvent {
            contact_id,
            direction: None,
            operation: TransportOperation::Connect,
            phase: OperationPhase::Completed,
            correlation_id: None,
            at: now,
            stage: Some(TransportStage::Connect),
            error_code: None,
        });
        self.observe_with_stage(TelemetryEvent {
            contact_id,
            direction: Some(TransportDirection::Tx),
            operation: TransportOperation::Handshake,
            phase: OperationPhase::Started,
            correlation_id: None,
            at: now,
            stage: Some(TransportStage::Handshake),
            error_code: None,
        });
        self.outgoing.insert(contact_id, session);
        Ok(())
    }
}
