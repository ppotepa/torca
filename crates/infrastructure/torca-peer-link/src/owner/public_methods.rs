impl<S, K> PeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    fn current_route_advertisement(&self) -> Option<torca_peer_protocol::RouteAdvertisement> {
        let route = self.transport_factory.local_route()?;
        torca_peer_protocol::RouteAdvertisement::new(
            route.provider.wire_value(),
            route.generation,
            route.endpoint,
        )
        .ok()
    }

    /// Sends the current provider route once per generation on a ready
    /// session. The route itself is opaque to the peer link; only the selected
    /// provider creates and interprets its bytes.
    fn advertise_route(
        &mut self,
        contact_id: ContactId,
        session: &mut PeerSession<Box<dyn PeerTransport + Send>, Ed25519HandshakeVerifier>,
    ) -> Result<(), PeerLinkError> {
        let Some(route) = self.current_route_advertisement() else { return Ok(()) };
        let already_sent = self
            .advertised_route_generations
            .get(&contact_id)
            .is_some_and(|generation| *generation >= route.generation);
        if already_sent {
            return Ok(());
        }
        session.send_route(route.clone()).map_err(map_session)?;
        self.advertised_route_generations.insert(contact_id, route.generation);
        Ok(())
    }

    fn advertise_route_for_contact(&mut self, contact_id: ContactId) -> Result<(), PeerLinkError> {
        let Some(route) = self.current_route_advertisement() else { return Ok(()) };
        if self.connection_state(contact_id) != PeerConnectionState::Ready {
            return Ok(());
        }
        if self
            .advertised_route_generations
            .get(&contact_id)
            .is_some_and(|generation| *generation >= route.generation)
        {
            return Ok(());
        }
        let result = if let Some(session) = self.outgoing.get_mut(&contact_id) {
            session.send_route(route.clone()).map_err(map_session)
        } else if let Some(session) = self.incoming.get_mut(&contact_id) {
            session.send_route(route.clone()).map_err(map_session)
        } else {
            Ok(())
        };
        result?;
        self.advertised_route_generations.insert(contact_id, route.generation);
        Ok(())
    }

    fn apply_route_advertisement(
        &mut self,
        contact_id: ContactId,
        route: torca_peer_protocol::RouteAdvertisement,
        now: Timestamp,
    ) -> Result<(), PeerLinkError> {
        if route.provider != self.transport_factory.kind().wire_value() {
            return Err(PeerLinkError::Protocol);
        }
        if self
            .route_generations
            .get(&contact_id)
            .is_some_and(|generation| *generation >= route.generation)
        {
            return Ok(());
        }
        let mut contact = self
            .relationships
            .get(contact_id)
            .map_err(map_contact)?
            .ok_or(PeerLinkError::ContactNotFound)?;
        let mut contact_route = contact.route().clone();
        contact_route
            .update_provider_endpoint(route.provider, route.endpoint)
            .map_err(map_contact)?;
        contact.update_route(contact_route, now).map_err(map_contact)?;
        self.relationships.update(contact).map_err(map_contact)?;
        self.route_generations.insert(contact_id, route.generation);
        self.notify_waker();
        Ok(())
    }

    /// Builds a peer link with an externally supplied transport factory.
    pub fn with_transport_factory(
        transport_factory: Box<dyn PeerTransportFactory>,
        relationships: S,
        signer: K,
        local_identity_id: OpaqueId,
    ) -> Self {
        Self {
            transport_factory,
            relationships,
            signer,
            local_identity_id,
            random: RustCryptoProvider,
            pending: Vec::new(),
            incoming: BTreeMap::new(),
            outgoing: BTreeMap::new(),
            reconnect: BTreeMap::new(),
            pending_acks: BTreeMap::new(),
            inbound: VecDeque::new(),
            activity: BTreeMap::new(),
            route_generations: BTreeMap::new(),
            advertised_route_generations: BTreeMap::new(),
            connectivity: None,
            waker: None,
        }
    }

    #[must_use]
    pub fn with_connectivity(mut self, connectivity: ConnectivityObserver) -> Self {
        self.connectivity = Some(connectivity);
        self
    }

    /// Returns the single provider selected for every session owned by this link.
    pub fn transport_kind(&self) -> TransportKind {
        self.transport_factory.kind()
    }

    /// Returns redaction-safe capabilities for diagnostics and policy decisions.
    pub fn transport_capabilities(&self) -> TransportCapabilities {
        self.transport_factory.capabilities()
    }

    pub fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) -> Result<(), PeerLinkError> {
        self.transport_factory.set_waker(Arc::clone(&waker)).map_err(map_transport_factory)?;
        for session in self.incoming.values_mut() {
            session.set_waker(Arc::clone(&waker));
        }
        for session in self.outgoing.values_mut() {
            session.set_waker(Arc::clone(&waker));
        }
        self.waker = Some(waker);
        Ok(())
    }

    fn notify_waker(&self) {
        if let Some(waker) = &self.waker {
            waker();
        }
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

    /// Starts a one-shot warm-up for active contacts after a relationship has
    /// just been created or restored. This is deliberately explicit: normal
    /// maintenance does not keep every contact connected, preserving the lazy
    /// connection policy for idle/background use.
    pub fn prime_connections(&mut self) -> Result<usize, PeerLinkError> {
        let contacts = self.relationships.list().map_err(map_contact)?;
        let now = system_timestamp()?;
        let mut started = 0;
        for contact in contacts {
            if contact.status() != ContactStatus::Active {
                continue;
            }
            if self.connection_state(contact.id()) == PeerConnectionState::Disconnected
                && self.preferred_dialer(contact.id())
            {
                self.reconnect.entry(contact.id()).or_insert(ReconnectEntry {
                    failures: 0,
                    next_attempt_at: now,
                    in_progress: false,
                });
                started += 1;
            }
        }
        Ok(started)
    }

    /// Starts a durable-demand dial for one relationship. This intentionally
    /// bypasses the preferred-dialer election: the outbox owner must be able
    /// to deliver even when the peer's reciprocal idle policy would normally
    /// leave this side passive.
    pub fn prime_contact(&mut self, contact_id: ContactId) -> Result<bool, PeerLinkError> {
        let contact = self
            .relationships
            .get(contact_id)
            .map_err(map_contact)?
            .ok_or(PeerLinkError::ContactNotFound)?;
        if contact.status() != ContactStatus::Active
            || self.connection_state(contact_id) != PeerConnectionState::Disconnected
        {
            return Ok(false);
        }
        let now = system_timestamp()?;
        self.reconnect.entry(contact.id()).or_insert(ReconnectEntry {
            failures: 0,
            next_attempt_at: now,
            in_progress: false,
        });
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
        let route_contacts = self
            .incoming
            .keys()
            .chain(self.outgoing.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        for contact_id in route_contacts {
            self.advertise_route_for_contact(contact_id)?;
        }
        self.plan_disconnected(contacts);
        self.run_due_reconnects(now, &mut report)?;
        if report.became_ready > 0
            || report.disconnected > 0
            || report.reconnect_started > 0
            || report.inbound_queued > 0
        {
            self.notify_waker();
        }
        Ok(report)
    }

    /// Closes quiet ready streams which have no active runtime demand.  Inbound
    /// queued work is always preserved; the next durable delivery or attention
    /// lease will schedule a reconnect without losing relationship state.
    pub fn close_idle_sessions(
        &mut self,
        retained: &[ContactId],
        now: Timestamp,
    ) -> Result<usize, PeerLinkError> {
        const IDLE_LIMIT: Duration = Duration::from_secs(10 * 60);
        let cutoff = now.checked_sub(IDLE_LIMIT);
        let queued = self.inbound.iter().map(|item| item.contact_id).collect::<BTreeSet<_>>();
        let should_close =
            |contact_id: ContactId, activity: &BTreeMap<ContactId, PeerActivitySnapshot>| {
                !retained.contains(&contact_id)
                    && !queued.contains(&contact_id)
                    && activity
                        .get(&contact_id)
                        .and_then(|entry| entry.last_activity_at)
                        .zip(cutoff)
                        .is_some_and(|(last, cutoff)| last <= cutoff)
            };
        let mut closed = 0;
        for contact_id in
            self.outgoing.keys().chain(self.incoming.keys()).copied().collect::<BTreeSet<_>>()
        {
            if !should_close(contact_id, &self.activity) {
                continue;
            }
            if let Some(mut session) = self.outgoing.remove(&contact_id) {
                session.close().map_err(map_session)?;
            }
            if let Some(mut session) = self.incoming.remove(&contact_id) {
                session.close().map_err(map_session)?;
            }
            self.reconnect.remove(&contact_id);
            closed += 1;
        }
        if closed > 0 {
            self.notify_waker();
        }
        Ok(closed)
    }

    pub fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        let reconnect_delay = self
            .reconnect
            .values()
            .filter(|entry| !entry.in_progress)
            .filter_map(|entry| entry.next_attempt_at.duration_since(now))
            .min();
        // Ready sessions have a blocking reader that wakes the runtime actor on
        // data or disconnect. Only pending/handshaking sessions need a deadline;
        // keeping ready sockets in this calculation would recreate the old
        // periodic `WouldBlock` hot loop.
        let active = !self.pending.is_empty()
            || self.incoming.values().any(|session| session.state() != PeerSessionState::Ready)
            || self.outgoing.values().any(|session| session.state() != PeerSessionState::Ready);
        match (active, reconnect_delay) {
            (true, Some(delay)) => Some(delay.min(Duration::from_millis(250))),
            (true, None) => Some(Duration::from_millis(250)),
            (false, delay) => delay,
        }
    }

    pub fn network_changed(&mut self, now: Timestamp) {
        // QUIC providers can migrate an authenticated session in place. Do
        // not tear those sessions down on every Wi-Fi/LTE callback: doing so
        // loses the very channel needed to advertise a refreshed route and
        // creates a reconnect burst. Legacy stream providers retain the
        // conservative close-and-reconnect behavior below.
        if self.transport_factory.preserves_sessions_on_network_change() {
            self.notify_waker();
            return;
        }
        let had_session =
            self.incoming.keys().chain(self.outgoing.keys()).copied().collect::<BTreeSet<_>>();
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
            let preferred = self.preferred_dialer(contact_id);
            if !preferred && !had_session.contains(&contact_id) {
                continue;
            }
            let next_attempt_at = if preferred {
                now
            } else {
                now.checked_add(Duration::from_secs(20)).unwrap_or(now)
            };
            self.reconnect.insert(
                contact_id,
                ReconnectEntry { failures: 0, next_attempt_at, in_progress: false },
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
        let deadline = Instant::now().checked_add(wait_slice).ok_or(PeerLinkError::AckTimeout)?;
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
            if matches!(self.ensure_connected(contact_id, now), Ok(true)) {
                self.notify_waker();
            }
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

    /// Records an ACK timeout without performing another blocking socket wait.
    /// SharedPeerLink uses this after releasing its mutex between polls, so a
    /// stalled peer cannot prevent lifecycle, delivery or Radio commands from
    /// acquiring the link.
    pub fn mark_ack_timeout(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), PeerLinkError> {
        self.mark_disconnected(contact_id);
        self.schedule_reconnect(contact_id, now)
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
            AckWaitAction::Route(route) => {
                self.apply_route_advertisement(contact_id, route, now)?;
                Ok(None)
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

    /// Sends several envelopes over one authenticated session operation. The
    /// durable delivery workers still receive one ACK per envelope; only
    /// provider framing is coalesced to avoid repeated mobile radio wakeups.
    pub fn send_envelopes_batch(
        &mut self,
        contact_id: ContactId,
        envelopes: Vec<(OpaqueId, u16, Vec<u8>)>,
    ) -> Result<(), PeerLinkError> {
        if envelopes.is_empty() {
            return Ok(());
        }
        if !self.is_ready(contact_id) {
            let now = system_timestamp()?;
            if matches!(self.ensure_connected(contact_id, now), Ok(true)) {
                self.notify_waker();
            }
            return Err(PeerLinkError::NotReady);
        }
        if let Some(session) = self.outgoing.get_mut(&contact_id) {
            if session.state() == PeerSessionState::Ready {
                return session.send_data_batch(envelopes).map_err(map_session);
            }
        }
        if let Some(session) = self.incoming.get_mut(&contact_id) {
            if session.state() == PeerSessionState::Ready {
                return session.send_data_batch(envelopes).map_err(map_session);
            }
        }
        Err(PeerLinkError::NotReady)
    }
}
