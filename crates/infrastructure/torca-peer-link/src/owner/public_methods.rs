pub const fn new(
    listener: PeerListener,
    relationships: S,
    signer: K,
    local_identity_id: OpaqueId,
    tor_client: TorServiceHandle,
) -> Self {
    Self {
        listener,
        relationships,
        signer,
        local_identity_id,
        tor_client,
        random: RustCryptoProvider,
        pending: Vec::new(),
        incoming: BTreeMap::new(),
        outgoing: BTreeMap::new(),
        reconnect: BTreeMap::new(),
        pending_acks: BTreeMap::new(),
        inbound: VecDeque::new(),
        activity: BTreeMap::new(),
        connectivity: None,
    }
}

#[must_use]
pub fn with_connectivity(mut self, connectivity: ConnectivityObserver) -> Self {
    self.connectivity = Some(connectivity);
    self
}

pub fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), PeerLinkError> {
    self.listener.set_waker(waker).map_err(|_| PeerLinkError::Listener)
}

pub fn connection_state(&self, contact_id: ContactId) -> PeerConnectionState {
    if let Some(session) = self.outgoing.get(&contact_id) {
        return map_state(session.state());
    }
    if let Some(session) = self.incoming.get(&contact_id) {
        return map_state(session.state());
    }
    PeerConnectionState::Disconnected
}

pub fn activity(&self) -> BTreeMap<ContactId, PeerActivitySnapshot> {
    self.activity.clone()
}

pub fn is_ready(&self, contact_id: ContactId) -> bool {
    self.connection_state(contact_id) == PeerConnectionState::Ready
}

pub fn ensure_connected(
    &mut self,
    contact_id: ContactId,
    now: Timestamp,
) -> Result<bool, PeerLinkError> {
    if matches!(
        self.connection_state(contact_id),
        PeerConnectionState::Connecting
            | PeerConnectionState::Handshaking
            | PeerConnectionState::Ready
    ) {
        return Ok(false);
    }
    self.remove_non_ready(contact_id);
    self.connect_outgoing(contact_id, now)?;
    Ok(true)
}

pub fn maintenance(
    &mut self,
    contacts: &[ContactId],
    now: Timestamp,
) -> Result<PeerLinkReport, PeerLinkError> {
    let mut report = PeerLinkReport::default();
    self.accept_pending(&mut report)?;
    self.authenticate_pending(now, &mut report)?;
    self.poll_sessions(now, &mut report)?;
    self.plan_disconnected(contacts);
    self.run_due_reconnects(now, &mut report)?;
    Ok(report)
}

pub fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
    let reconnect_delay = self
        .reconnect
        .values()
        .filter(|entry| !entry.in_progress)
        .filter_map(|entry| entry.next_attempt_at.duration_since(now))
        .min();
    let active =
        !self.pending.is_empty() || !self.incoming.is_empty() || !self.outgoing.is_empty();
    match (active, reconnect_delay) {
        (true, Some(delay)) => Some(delay.min(Duration::from_millis(250))),
        (true, None) => Some(Duration::from_millis(250)),
        (false, delay) => delay,
    }
}

pub fn network_changed(&mut self, now: Timestamp) {
    let mut contacts = self
        .incoming
        .keys()
        .chain(self.outgoing.keys())
        .chain(self.reconnect.keys())
        .copied()
        .collect::<Vec<_>>();
    contacts.sort_unstable();
    contacts.dedup();
    for (_, mut session) in std::mem::take(&mut self.incoming) {
        let _ = session.close();
    }
    for (_, mut session) in std::mem::take(&mut self.outgoing) {
        let _ = session.close();
    }
    self.pending.clear();
    self.pending_acks.clear();
    self.reconnect.clear();
    for contact_id in contacts {
        self.reconnect.insert(
            contact_id,
            ReconnectEntry {
                failures: 0,
                next_attempt_at: now,
                in_progress: false,
            },
        );
    }
}

pub fn send_and_wait_ack(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    message_kind: u16,
    ciphertext: Vec<u8>,
    timeout: Duration,
) -> Result<LinkAck, PeerLinkError> {
    self.send_and_wait_ack_with_limit(
        contact_id,
        envelope_id,
        message_kind,
        ciphertext,
        timeout,
        MAX_ACK_WAIT_SLICE,
    )
}

pub fn send_and_wait_ack_with_limit(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    message_kind: u16,
    ciphertext: Vec<u8>,
    timeout: Duration,
    wait_limit: Duration,
) -> Result<LinkAck, PeerLinkError> {
    self.send_envelope(contact_id, envelope_id, message_kind, ciphertext)?;
    let wait_slice = timeout.min(wait_limit);
    let deadline = Instant::now()
        .checked_add(wait_slice)
        .ok_or(PeerLinkError::AckTimeout)?;
    loop {
        if let Some(ack) = self.poll_envelope_ack_waiting(
            contact_id,
            envelope_id,
            deadline.saturating_duration_since(Instant::now()),
        )? {
            return Ok(ack);
        }
        if Instant::now() >= deadline {
            let now = system_timestamp()?;
            if timeout <= MAX_ACK_WAIT_SLICE {
                self.mark_disconnected(contact_id);
                self.schedule_reconnect(contact_id, now)?;
            }
            return Err(PeerLinkError::AckTimeout);
        }
    }
}

pub fn send_envelope(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    message_kind: u16,
    ciphertext: Vec<u8>,
) -> Result<(), PeerLinkError> {
    if !self.is_ready(contact_id) {
        let now = system_timestamp()?;
        let _ = self.ensure_connected(contact_id, now);
        return Err(PeerLinkError::NotReady);
    }
    let started_at = system_timestamp()?;
    self.observe(
        contact_id,
        Some(TransportDirection::Tx),
        TransportOperation::Envelope,
        OperationPhase::Started,
        Some(envelope_id),
        started_at,
    );
    if let Err(error) = self.send_data(contact_id, envelope_id, message_kind, ciphertext) {
        self.observe(
            contact_id,
            Some(TransportDirection::Tx),
            TransportOperation::Envelope,
            OperationPhase::Failed,
            Some(envelope_id),
            started_at,
        );
        return Err(error);
    }
    self.observe(
        contact_id,
        Some(TransportDirection::Tx),
        TransportOperation::Envelope,
        OperationPhase::Completed,
        Some(envelope_id),
        started_at,
    );
    Ok(())
}

pub fn poll_envelope_ack(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
) -> Result<Option<LinkAck>, PeerLinkError> {
    self.poll_envelope_ack_waiting(contact_id, envelope_id, Duration::ZERO)
}

fn poll_envelope_ack_waiting(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    wait: Duration,
) -> Result<Option<LinkAck>, PeerLinkError> {
    if let Some(ack) = self.pending_acks.remove(&(contact_id, envelope_id)) {
        return ack.map(Some);
    }
    let now = system_timestamp()?;
    let message = if wait.is_zero() {
        self.poll_contact(contact_id, now)
    } else {
        self.poll_contact_wait(contact_id, now, wait)
    };
    let action = match message {
        Ok(Some(message)) => classify_ack_wait_message(contact_id, envelope_id, message),
        Ok(None) => AckWaitAction::Ignore,
        Err(error) => {
            self.schedule_reconnect(contact_id, now)?;
            return Err(error);
        }
    };
    match action {
        AckWaitAction::Complete(ack) => {
            self.observe(
                contact_id,
                Some(TransportDirection::Rx),
                TransportOperation::Ack,
                OperationPhase::Completed,
                Some(envelope_id),
                now,
            );
            ack.map(Some)
        }
        AckWaitAction::Store { envelope_id: received, ack } => {
            if self.pending_acks.len() >= MAX_PENDING_ACKS {
                if let Some(oldest) = self.pending_acks.keys().next().copied() {
                    self.pending_acks.remove(&oldest);
                }
            }
            self.pending_acks.insert((contact_id, received), ack);
            Ok(None)
        }
        AckWaitAction::QueueInbound(envelope) => {
            let received = envelope.envelope_id;
            self.observe(
                contact_id,
                Some(TransportDirection::Rx),
                TransportOperation::Envelope,
                OperationPhase::Completed,
                Some(received),
                now,
            );
            self.queue_inbound(envelope)?;
            Err(PeerLinkError::InboundPending)
        }
        AckWaitAction::Ignore => Ok(None),
    }
}

pub fn send_keepalive_and_wait_ack(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    message_kind: u16,
    ciphertext: Vec<u8>,
    timeout: Duration,
) -> Result<LinkAck, PeerLinkError> {
    let started_at = system_timestamp()?;
    self.observe(
        contact_id,
        Some(TransportDirection::Tx),
        TransportOperation::Keepalive,
        OperationPhase::Started,
        Some(envelope_id),
        started_at,
    );
    let result = match self.send_and_wait_ack(
        contact_id,
        envelope_id,
        message_kind,
        ciphertext,
        timeout,
    ) {
        Err(PeerLinkError::InboundPending) => Ok(LinkAck::Accepted),
        result => result,
    };
    let finished_at = system_timestamp().unwrap_or(started_at);
    self.observe(
        contact_id,
        Some(TransportDirection::Rx),
        TransportOperation::Keepalive,
        if result.is_ok() { OperationPhase::Completed } else { OperationPhase::TimedOut },
        Some(envelope_id),
        finished_at,
    );
    result
}

pub fn send_ack(
    &mut self,
    contact_id: ContactId,
    envelope_id: OpaqueId,
    status: AckStatus,
) -> Result<(), PeerLinkError> {
    if let Some(session) = self.outgoing.get_mut(&contact_id) {
        if session.state() == PeerSessionState::Ready {
            let result = session.send_ack(envelope_id, status).map_err(map_session);
            self.observe_send_ack(contact_id, envelope_id, &result);
            return result;
        }
    }
    if let Some(session) = self.incoming.get_mut(&contact_id) {
        if session.state() == PeerSessionState::Ready {
            let result = session.send_ack(envelope_id, status).map_err(map_session);
            self.observe_send_ack(contact_id, envelope_id, &result);
            return result;
        }
    }
    Err(PeerLinkError::NotReady)
}

pub fn take_inbound(&mut self) -> Option<InboundPeerEnvelope> {
    self.inbound.pop_front()
}

pub fn shutdown(&mut self) {
    for mut transport in self.pending.drain(..) {
        let _ = transport.close();
    }
    for (_, mut session) in std::mem::take(&mut self.incoming) {
        let _ = session.close();
    }
    for (_, mut session) in std::mem::take(&mut self.outgoing) {
        let _ = session.close();
    }
    self.reconnect.clear();
    self.inbound.clear();
}

pub fn into_parts(self) -> (PeerListener, S, K) {
    (self.listener, self.relationships, self.signer)
}
