impl<S, K> PeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
fn poll_sessions(
    &mut self,
    now: Timestamp,
    report: &mut PeerLinkReport,
) -> Result<(), PeerLinkError> {
    let outgoing_ids: Vec<_> = self.outgoing.keys().copied().collect();
    for contact_id in outgoing_ids {
        let was_ready = self
            .outgoing
            .get(&contact_id)
            .is_some_and(|session| session.state() == PeerSessionState::Ready);
        match self.poll_contact(contact_id, now) {
            Ok(Some(PeerMessage::Data {
                envelope_id,
                message_kind,
                ciphertext,
            })) => {
                self.observe(
                    contact_id,
                    Some(TransportDirection::Rx),
                    TransportOperation::Envelope,
                    OperationPhase::Completed,
                    Some(envelope_id),
                    now,
                );
                self.queue_inbound(InboundPeerEnvelope {
                    contact_id,
                    envelope_id,
                    message_kind,
                    ciphertext,
                })?;
                report.inbound_queued += 1;
            }
            Ok(Some(PeerMessage::Ack { envelope_id, status })) => {
                self.observe(
                    contact_id,
                    Some(TransportDirection::Rx),
                    TransportOperation::Ack,
                    OperationPhase::Completed,
                    Some(envelope_id),
                    now,
                );
                self.store_pending_ack(contact_id, envelope_id, status);
            }
            Ok(_) => {
                if !was_ready && self.is_ready(contact_id) {
                    self.reconnect.remove(&contact_id);
                    self.observe(
                        contact_id,
                        Some(TransportDirection::Rx),
                        TransportOperation::Handshake,
                        OperationPhase::Completed,
                        None,
                        now,
                    );
                    report.became_ready += 1;
                }
            }
            Err(_) => {
                self.schedule_reconnect(contact_id, now)?;
                report.disconnected += 1;
            }
        }
    }

    let incoming_ids: Vec<_> = self.incoming.keys().copied().collect();
    for contact_id in incoming_ids {
        match self.poll_contact(contact_id, now) {
            Ok(Some(PeerMessage::Data {
                envelope_id,
                message_kind,
                ciphertext,
            })) => {
                self.observe(
                    contact_id,
                    Some(TransportDirection::Rx),
                    TransportOperation::Envelope,
                    OperationPhase::Completed,
                    Some(envelope_id),
                    now,
                );
                self.queue_inbound(InboundPeerEnvelope {
                    contact_id,
                    envelope_id,
                    message_kind,
                    ciphertext,
                })?;
                report.inbound_queued += 1;
            }
            Ok(Some(PeerMessage::Ack { envelope_id, status })) => {
                self.observe(
                    contact_id,
                    Some(TransportDirection::Rx),
                    TransportOperation::Ack,
                    OperationPhase::Completed,
                    Some(envelope_id),
                    now,
                );
                self.store_pending_ack(contact_id, envelope_id, status);
            }
            Ok(_) => {}
            Err(_) => {
                self.mark_disconnected(contact_id);
                if self
                    .relationships
                    .get(contact_id)
                    .map_err(map_contact)?
                    .is_some_and(|contact| self.prefer_outgoing(&contact))
                {
                    self.schedule_reconnect(contact_id, now)?;
                }
                report.disconnected += 1;
            }
        }
    }
    Ok(())
}

fn store_pending_ack(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    status: AckStatus,
) {
    let ack = link_ack(status);
    if self.pending_acks.len() >= MAX_PENDING_ACKS {
        if let Some(oldest) = self.pending_acks.keys().next().copied() {
            self.pending_acks.remove(&oldest);
        }
    }
    self.pending_acks.insert((contact_id, envelope_id), ack);
}

fn poll_contact(
    &mut self,
    contact_id: ContactId,
    now: Timestamp,
) -> Result<Option<PeerMessage>, PeerLinkError> {
    if let Some(session) = self.outgoing.get_mut(&contact_id) {
        return session.poll(now).map_err(map_session);
    }
    if let Some(session) = self.incoming.get_mut(&contact_id) {
        return session.poll(now).map_err(map_session);
    }
    Ok(None)
}

fn poll_contact_wait(
    &mut self,
    contact_id: ContactId,
    now: Timestamp,
    timeout: Duration,
) -> Result<Option<PeerMessage>, PeerLinkError> {
    if let Some(session) = self.outgoing.get_mut(&contact_id) {
        return session.wait_poll(now, timeout).map_err(map_session);
    }
    if let Some(session) = self.incoming.get_mut(&contact_id) {
        return session.wait_poll(now, timeout).map_err(map_session);
    }
    Ok(None)
}

fn send_data(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    message_kind: u16,
    ciphertext: Vec<u8>,
) -> Result<(), PeerLinkError> {
    if let Some(session) = self.outgoing.get_mut(&contact_id) {
        if session.state() == PeerSessionState::Ready {
            return session
                .send_data(envelope_id, message_kind, ciphertext)
                .map_err(map_session);
        }
    }
    if let Some(session) = self.incoming.get_mut(&contact_id) {
        if session.state() == PeerSessionState::Ready {
            return session
                .send_data(envelope_id, message_kind, ciphertext)
                .map_err(map_session);
        }
    }
    Err(PeerLinkError::NotReady)
}

fn queue_inbound(&mut self, envelope: InboundPeerEnvelope) -> Result<(), PeerLinkError> {
    if self.inbound.len() >= MAX_INBOUND_EVENTS {
        return Err(PeerLinkError::InboundQueueFull);
    }
    self.inbound.push_back(envelope);
    Ok(())
}

fn plan_disconnected(&mut self, contacts: &[ContactId]) {
    for &contact_id in contacts {
        if !self.preferred_dialer(contact_id) {
            self.reconnect.remove(&contact_id);
            continue;
        }
        match self.connection_state(contact_id) {
            PeerConnectionState::Ready => {
                self.reconnect.remove(&contact_id);
            }
            PeerConnectionState::Disconnected
            | PeerConnectionState::Reconnecting
            | PeerConnectionState::Failed => {}
            PeerConnectionState::Connecting | PeerConnectionState::Handshaking => {}
        }
    }
}

fn run_due_reconnects(
    &mut self,
    now: Timestamp,
    report: &mut PeerLinkReport,
) -> Result<(), PeerLinkError> {
    let due: Vec<_> = self
        .reconnect
        .iter()
        .filter_map(|(contact_id, entry)| {
            (!entry.in_progress && entry.next_attempt_at <= now).then_some(*contact_id)
        })
        .collect();
    for contact_id in due {
        if self.is_ready(contact_id) {
            self.reconnect.remove(&contact_id);
            continue;
        }
        self.remove_non_ready(contact_id);
        match self.connect_outgoing(contact_id, now) {
            Ok(()) => {
                if let Some(entry) = self.reconnect.get_mut(&contact_id) {
                    entry.in_progress = true;
                }
                report.reconnect_started += 1;
            }
            Err(_) => self.schedule_reconnect(contact_id, now)?,
        }
    }
    Ok(())
}

fn schedule_reconnect(
    &mut self,
    contact_id: ContactId,
    now: Timestamp,
) -> Result<(), PeerLinkError> {
    if self.is_ready(contact_id) {
        self.reconnect.remove(&contact_id);
        return Ok(());
    }
    let failures = self
        .reconnect
        .get(&contact_id)
        .map_or(1, |entry| entry.failures.saturating_add(1));
    let delay = reconnect_delay(&mut self.random, failures)?;
    let next_attempt_at = now.checked_add(delay).ok_or(PeerLinkError::Clock)?;
    self.observe(
        contact_id,
        None,
        TransportOperation::Reconnect,
        OperationPhase::Started,
        None,
        now,
    );
    self.reconnect.insert(
        contact_id,
        ReconnectEntry {
            failures,
            next_attempt_at,
            in_progress: false,
        },
    );
    Ok(())
}

fn remove_non_ready(&mut self, contact_id: ContactId) {
    if self
        .outgoing
        .get(&contact_id)
        .is_some_and(|session| session.state() != PeerSessionState::Ready)
    {
        if let Some(mut session) = self.outgoing.remove(&contact_id) {
            let _ = session.close();
        }
    }
    if self
        .incoming
        .get(&contact_id)
        .is_some_and(|session| session.state() != PeerSessionState::Ready)
    {
        if let Some(mut session) = self.incoming.remove(&contact_id) {
            let _ = session.close();
        }
    }
}

fn mark_disconnected(&mut self, contact_id: ContactId) {
    if let Some(session) = self.outgoing.get_mut(&contact_id) {
        session.disconnected();
    }
    if let Some(session) = self.incoming.get_mut(&contact_id) {
        session.disconnected();
    }
}

fn contact_for_identity(&self, identity_id: OpaqueId) -> Result<Contact, PeerLinkError> {
    self.relationships
        .list()
        .map_err(map_contact)?
        .into_iter()
        .find(|contact| contact.remote_identity().identity_id().to_opaque() == identity_id)
        .ok_or(PeerLinkError::Unauthorized)
}

fn prefer_outgoing(&self, contact: &Contact) -> bool {
    self.local_identity_id < contact.remote_identity().identity_id().to_opaque()
}

fn preferred_dialer(&self, contact_id: ContactId) -> bool {
    self.relationships
        .get(contact_id)
        .ok()
        .flatten()
        .is_some_and(|contact| self.prefer_outgoing(&contact))
}

fn random_id(&mut self) -> Result<OpaqueId, PeerLinkError> {
    for _ in 0..8 {
        let mut bytes = [0_u8; 16];
        self.random
            .fill_random(&mut bytes)
            .map_err(|_| PeerLinkError::Randomness)?;
        let id = OpaqueId::from_bytes(bytes);
        if !id.is_nil() {
            return Ok(id);
        }
    }
    Err(PeerLinkError::Randomness)
}

fn random_32(&mut self) -> Result<[u8; 32], PeerLinkError> {
    let mut bytes = [0_u8; 32];
    self.random
        .fill_random(&mut bytes)
        .map_err(|_| PeerLinkError::Randomness)?;
    Ok(bytes)
}
}
