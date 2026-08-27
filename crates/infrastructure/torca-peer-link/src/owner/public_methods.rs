impl<S, K> PeerLink<S, K>
where
    S: ContactRepository + PeerCredentialRepository,
    K: HandshakeSigner,
{
    fn current_route_advertisement(&self) -> Option<torca_peer_protocol::RouteAdvertisement> {
        if let Some(routing) = &self.provider_routing {
            let route = routing.local_route().ok().flatten()?;
            return torca_peer_protocol::RouteAdvertisement::new(
                route.provider.into_string(),
                route.generation,
                route.endpoint,
            )
            .ok();
        }
        None
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
        if let Ok(now) = system_timestamp() {
            self.observe_with_stage(TelemetryEvent {
                contact_id,
                direction: Some(TransportDirection::Tx),
                operation: TransportOperation::Route,
                phase: OperationPhase::Completed,
                correlation_id: None,
                at: now,
                stage: Some(TransportStage::RouteAdvertised),
                error_code: None,
            });
        }
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
        if let Ok(now) = system_timestamp() {
            self.observe_with_stage(TelemetryEvent {
                contact_id,
                direction: Some(TransportDirection::Tx),
                operation: TransportOperation::Route,
                phase: OperationPhase::Completed,
                correlation_id: None,
                at: now,
                stage: Some(TransportStage::RouteAdvertised),
                error_code: None,
            });
        }
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
        self.observe_with_stage(TelemetryEvent {
            contact_id,
            direction: Some(TransportDirection::Rx),
            operation: TransportOperation::Route,
            phase: OperationPhase::Completed,
            correlation_id: None,
            at: now,
            stage: Some(TransportStage::RouteApplied),
            error_code: None,
        });
        self.observe_with_stage(TelemetryEvent {
            contact_id,
            direction: None,
            operation: TransportOperation::Route,
            phase: OperationPhase::Completed,
            correlation_id: None,
            at: now,
            stage: Some(TransportStage::RouteRefreshed),
            error_code: None,
        });
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
        Self::with_optional_provider_routing(
            transport_factory,
            None,
            relationships,
            signer,
            local_identity_id,
        )
    }

    /// Builds a peer link whose authenticated route advertisements share the
    /// same provider owner as pairing bootstrap.
    pub fn with_provider_routing(
        transport_factory: Box<dyn PeerTransportFactory>,
        provider_routing: std::sync::Arc<dyn torca_provider_api::ProviderRouting>,
        relationships: S,
        signer: K,
        local_identity_id: OpaqueId,
    ) -> Self {
        Self::with_optional_provider_routing(
            transport_factory,
            Some(provider_routing),
            relationships,
            signer,
            local_identity_id,
        )
    }

    fn with_optional_provider_routing(
        transport_factory: Box<dyn PeerTransportFactory>,
        provider_routing: Option<std::sync::Arc<dyn torca_provider_api::ProviderRouting>>,
        relationships: S,
        signer: K,
        local_identity_id: OpaqueId,
    ) -> Self {
        Self {
            transport_factory,
            provider_routing,
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
            peer_recovery_started_at: None,
            peer_recovery_generation: 0,
            peer_recovery_attempts: 0,
            peer_recovery_exhausted: false,
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
                self.request_reconnect(
                    contact.id(),
                    now,
                    ReconnectReason::PreferredDialer,
                );
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
        self.request_reconnect(contact.id(), now, ReconnectReason::DurableDemand);
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
        self.expire_stuck_peer_recovery(now, &mut report)?;
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
        self.update_peer_recovery_state(now);
        if report.became_ready > 0
            || report.disconnected > 0
            || report.reconnect_started > 0
            || report.inbound_queued > 0
        {
            self.notify_waker();
        }
        Ok(report)
    }

    fn expire_stuck_peer_recovery(
        &mut self,
        now: Timestamp,
        report: &mut PeerLinkReport,
    ) -> Result<(), PeerLinkError> {
        let expired = self.peer_recovery_started_at.is_some()
            && peer_recovery_delay(self.peer_recovery_started_at, now).is_none();
        if !expired {
            return Ok(());
        }

        let contacts = self
            .incoming
            .keys()
            .chain(self.outgoing.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        for contact_id in contacts {
            if self.connection_state(contact_id) == PeerConnectionState::Ready {
                continue;
            }
            self.remove_non_ready(contact_id);
            self.schedule_reconnect(contact_id, now)?;
            report.disconnected += 1;
        }
        while let Some(mut transport) = self.pending.pop() {
            let _ = transport.close();
            report.disconnected += 1;
        }
        self.peer_recovery_started_at = None;
        self.peer_recovery_attempts = 0;
        self.peer_recovery_exhausted = false;
        Ok(())
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
        let recovery_delay = active
            .then(|| peer_recovery_delay(self.peer_recovery_started_at, now))
            .flatten();
        [reconnect_delay, recovery_delay].into_iter().flatten().min()
    }

    fn update_peer_recovery_state(&mut self, now: Timestamp) {
        let active = !self.pending.is_empty()
            || self.incoming.values().any(|session| session.state() != PeerSessionState::Ready)
            || self.outgoing.values().any(|session| session.state() != PeerSessionState::Ready);
        if !active {
            self.peer_recovery_started_at = None;
            self.peer_recovery_attempts = 0;
            self.peer_recovery_exhausted = false;
            return;
        }
        if self.peer_recovery_started_at.is_none() {
            self.peer_recovery_started_at = Some(now);
            self.peer_recovery_generation = self.peer_recovery_generation.saturating_add(1);
            self.peer_recovery_attempts = 0;
            self.peer_recovery_exhausted = false;
        }
        if peer_recovery_delay(self.peer_recovery_started_at, now).is_some() {
            self.peer_recovery_attempts = self.peer_recovery_attempts.saturating_add(1);
        } else {
            self.peer_recovery_exhausted = true;
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
        let reconnect_reasons = self
            .reconnect
            .iter()
            .map(|(contact_id, entry)| (*contact_id, entry.reason))
            .collect::<BTreeMap<_, _>>();
        self.reconnect.clear();
        for contact_id in contacts {
            let preferred = self.preferred_dialer(contact_id);
            let existing_reason = reconnect_reasons.get(&contact_id).copied();
            if !preferred && !had_session.contains(&contact_id) && existing_reason.is_none() {
                continue;
            }
            let reason = existing_reason.unwrap_or(if preferred {
                ReconnectReason::PreferredDialer
            } else {
                ReconnectReason::Recovery
            });
            let next_attempt_at = if preferred || reason == ReconnectReason::DurableDemand {
                now
            } else {
                now.checked_add(Duration::from_secs(20)).unwrap_or(now)
            };
            self.request_reconnect(contact_id, next_attempt_at, reason);
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

#[cfg(test)]
mod route_tests {
    use std::sync::Arc;

    use torca_contacts::{
        Contact, ContactError, ContactId, ContactRepository, ContactRoute, PeerCredential,
        PeerCredentialRepository,
    };
    use torca_foundation::{OpaqueId, Timestamp};
    use torca_identity::{
        IdentityId, IdentityKey, KeyAlgorithm, KeyId, PublicIdentity,
    };
    use torca_peer_protocol::{
        HandshakeSigner, HandshakeSigningError, RouteAdvertisement,
    };
    use torca_transport_api::{
        PeerTransport, PeerTransportFactory, TransportCapabilities, TransportFactoryError,
        TransportKind,
    };

    use super::PeerLink;
    use crate::PeerLinkError;

    struct RouteRelationships {
        contact: Contact,
    }

    impl ContactRepository for RouteRelationships {
        fn insert(&mut self, _contact: Contact) -> Result<(), ContactError> {
            Err(ContactError::AlreadyExists)
        }

        fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
            Ok((self.contact.id() == id).then(|| self.contact.clone()))
        }

        fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
            self.contact = contact;
            Ok(())
        }

        fn list(&self) -> Result<Vec<Contact>, ContactError> {
            Ok(vec![self.contact.clone()])
        }
    }

    impl PeerCredentialRepository for RouteRelationships {
        fn insert_credential(&mut self, _credential: PeerCredential) -> Result<(), ContactError> {
            Ok(())
        }

        fn credential_for_contact(
            &self,
            _contact_id: ContactId,
        ) -> Result<Option<PeerCredential>, ContactError> {
            Ok(None)
        }
    }

    struct RouteFactory;

    impl PeerTransportFactory for RouteFactory {
        fn kind(&self) -> TransportKind {
            TransportKind::Memory
        }

        fn capabilities(&self) -> TransportCapabilities {
            panic!("route update does not inspect capabilities")
        }

        fn accept(
            &mut self,
        ) -> Result<Option<Box<dyn PeerTransport + Send>>, TransportFactoryError> {
            Ok(None)
        }

        fn connect(
            &mut self,
            _contact: &Contact,
        ) -> Result<Box<dyn PeerTransport + Send>, TransportFactoryError> {
            Err(TransportFactoryError::Listener)
        }

        fn set_waker(
            &self,
            _waker: Arc<dyn Fn() + Send + Sync>,
        ) -> Result<(), TransportFactoryError> {
            Ok(())
        }
    }

    struct RouteSigner;

    impl HandshakeSigner for RouteSigner {
        fn sign(&self, _canonical: &[u8]) -> Result<Vec<u8>, HandshakeSigningError> {
            Ok(vec![0; 64])
        }
    }

    fn route_link() -> (PeerLink<RouteRelationships, RouteSigner>, ContactId) {
        let contact_id = ContactId::from_u128(9);
        let key = IdentityKey::new(KeyId::from_u128(3), KeyAlgorithm::Ed25519, vec![7; 32])
            .expect("valid peer key");
        let contact = Contact::new(
            contact_id,
            PublicIdentity::new(IdentityId::from_u128(4), key, 0),
            ContactRoute::for_provider_endpoint(
                OpaqueId::from_u128(5),
                TransportKind::Memory.wire_value(),
                b"initial".to_vec(),
            )
            .expect("valid initial route"),
            Timestamp::UNIX_EPOCH,
        );
        (
            PeerLink::with_transport_factory(
                Box::new(RouteFactory),
                RouteRelationships { contact },
                RouteSigner,
                OpaqueId::from_u128(6),
            ),
            contact_id,
        )
    }

    #[test]
    fn route_advertisements_are_monotonic_idempotent_and_provider_bound() {
        let (mut link, contact_id) = route_link();
        let at = Timestamp::from_unix_millis(1).expect("valid time");

        link.apply_route_advertisement(
            contact_id,
            RouteAdvertisement::new("memory", 2, b"generation-two".to_vec())
                .expect("valid route"),
            at,
        )
        .expect("new route is applied");
        assert_eq!(
            link.relationships
                .get(contact_id)
                .expect("read contact")
                .expect("persisted contact")
                .route()
                .provider_endpoint("memory"),
            Some(b"generation-two".as_slice())
        );

        for (generation, endpoint) in [
            (1, b"older".as_slice()),
            (2, b"same-generation".as_slice()),
        ] {
            link.apply_route_advertisement(
                contact_id,
                RouteAdvertisement::new("memory", generation, endpoint.to_vec())
                    .expect("valid route"),
                at,
            )
            .expect("old or repeated route is idempotent");
        }
        assert_eq!(
            link.relationships
                .get(contact_id)
                .expect("read contact")
                .expect("persisted contact")
                .route()
                .provider_endpoint("memory"),
            Some(b"generation-two".as_slice())
        );

        link.apply_route_advertisement(
            contact_id,
            RouteAdvertisement::new("memory", 3, b"generation-three".to_vec())
                .expect("valid route"),
            at,
        )
        .expect("newer route is applied");
        assert_eq!(
            link.relationships
                .get(contact_id)
                .expect("read contact")
                .expect("persisted contact")
                .route()
                .provider_endpoint("memory"),
            Some(b"generation-three".as_slice())
        );

        assert_eq!(
            link.apply_route_advertisement(
                contact_id,
                RouteAdvertisement::new("iroh", 4, b"wrong-provider".to_vec())
                    .expect("valid route frame"),
                at,
            ),
            Err(PeerLinkError::Protocol)
        );
    }
}
