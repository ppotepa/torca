pub trait PairingDriver: Send + 'static {
    fn create(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingInvitationView, RuntimeDriverError>;
    fn join(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
        bootstrap: Option<PairingBootstrapDescriptor>,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn approve(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError>;
    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError>;
    fn maintenance(
        &mut self,
        now: Timestamp,
    ) -> Result<PairingMaintenanceReport, RuntimeDriverError>;
    /// Returns the next useful maintenance deadline. `None` means the worker
    /// can sleep until a command or network event arrives; it must not wake
    /// just to discover that there is no pairing work.
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }
    fn network_changed(&mut self, _now: Timestamp) {}
    /// Installs an event-driven wake used when a background pairing worker
    /// persists a relationship between runtime maintenance turns.
    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}
    fn shutdown(&mut self);
}

impl PairingDriver for Box<dyn PairingDriver> {
    fn create(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        (**self).create(session_id, now)
    }

    fn join(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
        bootstrap: Option<PairingBootstrapDescriptor>,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        (**self).join(session_id, code, ticket, bootstrap, now)
    }

    fn approve(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        (**self).approve(session_id, now)
    }

    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        (**self).reject(session_id)
    }

    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        (**self).cancel(session_id)
    }

    fn maintenance(
        &mut self,
        now: Timestamp,
    ) -> Result<PairingMaintenanceReport, RuntimeDriverError> {
        (**self).maintenance(now)
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        (**self).next_maintenance_delay(now)
    }

    fn network_changed(&mut self, now: Timestamp) {
        (**self).network_changed(now);
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        (**self).set_waker(waker);
    }

    fn shutdown(&mut self) {
        (**self).shutdown();
    }
}
/// Owns only background delivery/inbound maintenance and peer session state.
pub trait PeerSessionPort: Send + 'static {
    fn recover(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    fn maintenance(
        &mut self,
        contacts: &[ContactId],
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    /// Returns the next durable communication deadline. `None` means that
    /// this adapter has no known retry deadline and can rely on an external
    /// wake (user action, inbound data or network change).
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }
    /// Invalidates stale transport sessions and resets reconnect backoff after
    /// an OS route/network change.
    fn network_changed(&mut self, _now: Timestamp) {}
    /// Primes newly-created relationships immediately after pairing. The
    /// default keeps lightweight test drivers compatible; production
    /// transports use this as an explicit, event-driven warm-up rather than
    /// waiting for the first user message to discover a disconnected peer.
    fn prime_connections(&mut self) {}
    /// Primes only the peer that owns a durable user-visible operation.
    /// Unlike relationship warm-up this is allowed to dial regardless of the
    /// deterministic preferred-dialer role: a queued message must make
    /// progress from the side that owns the outbox.
    fn prime_contact(&mut self, _contact_id: ContactId) {}
    /// Installs a non-blocking wake path for inbound listener activity.
    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}
    fn connection_state(&self, contact_id: ContactId) -> PeerConnectionStatus;
    fn peer_health(&self, contact_id: ContactId) -> PeerHealthSnapshot {
        PeerHealthSnapshot::from_connection_state(self.connection_state(contact_id))
    }
    /// Returns monotonic transport activity counters for policy evidence.
    fn peer_activity(&self) -> Vec<PeerActivityEvidence> {
        Vec::new()
    }
    fn close_idle_peers(
        &mut self,
        _retained: &[ContactId],
        _now: Timestamp,
    ) -> Result<usize, RuntimeDriverError> {
        Ok(0)
    }
    /// Whether this device is the deterministic initiator of the keepalive
    /// for this relationship. The adapter supplies the transport capability;
    /// application owns cadence and retry policy.
    fn peer_probe_eligible(&self, _contact_id: ContactId) -> bool {
        true
    }
    /// Starts one bounded keepalive I/O operation. Implementations must return
    /// promptly after accepting it into their single-flight worker.
    fn begin_peer_probe(
        &mut self,
        _contact_id: ContactId,
        _probe_id: OpaqueId,
        _reported_rtt_ms: u64,
    ) -> Result<(), RuntimeDriverError> {
        Ok(())
    }
    /// Returns the contact whose pending keepalive completed. Health details
    /// remain available through `peer_health`, avoiding infrastructure errors
    /// in the application vocabulary.
    fn take_peer_probe_completion(
        &mut self,
        _now: Timestamp,
    ) -> Result<Option<ContactId>, RuntimeDriverError> {
        Ok(None)
    }
    fn shutdown(&mut self);
}

/// Contact administration is not a transport command, despite some actions
/// causing a peer session to be closed by its infrastructure implementation.
pub trait RelationshipAdminPort: Send + 'static {
    fn contact_names(&self) -> Result<BTreeMap<ContactId, String>, RuntimeDriverError>;
    fn contact_verifications(
        &self,
    ) -> Result<BTreeMap<ContactId, ContactVerificationSnapshot>, RuntimeDriverError>;
    fn verify_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn reset_contact_verification(
        &mut self,
        contact_id: ContactId,
    ) -> Result<(), RuntimeDriverError>;
    fn rename_contact(
        &mut self,
        contact_id: ContactId,
        display_name: String,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn block_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn unblock_contact(
        &mut self,
        contact_id: ContactId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn clear_conversation_history(
        &mut self,
        conversation_id: ConversationId,
    ) -> Result<(), RuntimeDriverError>;
    fn remove_contact(&mut self, contact_id: ContactId) -> Result<(), RuntimeDriverError>;
}

pub trait ConversationReadPort: Send + 'static {
    fn mark_conversation_read(
        &mut self,
        conversation_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
}

pub trait AttachmentTransferPort: Send + 'static {
    fn set_battery_policy(
        &mut self,
        _profile: BatteryProfile,
        _metered_transfers: MeteredTransferPolicy,
        _metered_network: bool,
    ) {
    }
    fn prepare_attachment(
        &mut self,
        request: &AttachmentSendRequest,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn retry_attachment(
        &mut self,
        attachment_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn cancel_attachment(
        &mut self,
        attachment_id: OpaqueId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError>;

    /// Returns asynchronous preparation failures together with the generated
    /// message id. Preparation runs outside the runtime actor, so a failure
    /// must be reconciled explicitly instead of leaving an outbox message and
    /// attachment lease permanently pending.
    fn take_attachment_prepare_failures(&mut self) -> Vec<(OpaqueId, OpaqueId)> {
        Vec::new()
    }
}

pub trait AttachmentExportPort: Send + 'static {
    fn export_attachment(
        &mut self,
        attachment_id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError>;
    fn export_attachment_preview(
        &mut self,
        attachment_id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError>;
}

/// Compatibility composition for the process runtime. New use cases should
/// depend on one of the narrow ports above, not this aggregate.
pub trait CommunicationDriver:
    PeerSessionPort
    + RelationshipAdminPort
    + ConversationReadPort
    + AttachmentTransferPort
    + AttachmentExportPort
{
    fn queue_outbound(
        &mut self,
        _message: Message,
        _command_id: CommandId,
        _next_attempt_at: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        Err(RuntimeDriverError::Communication)
    }

    /// Wakes text/control outbox workers after a local durable mutation.
    /// This is separate from a scheduler deadline: a previously idle worker
    /// may have reported `None` and still needs an explicit command wake.
    fn wake_delivery(&mut self) {}

    /// Wakes only the Radio lane after provider media activity.  Keeping this
    /// separate from delivery/peer wakeups prevents a Radio frame from being
    /// mistaken for a text outbox deadline and guarantees that the runtime
    /// drains `RadioSessionEvent` promptly.
    fn set_radio_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}

    /// Runs only Radio control/media housekeeping. RuntimeOwner calls this
    /// from `RadioDeadline`; delivery work must not wake an idle Radio lane.
    fn maintain_radio(&mut self, _now: Timestamp) -> Result<(), RuntimeDriverError> {
        Ok(())
    }
    /// Returns a Radio-owned deadline separately from delivery/attachment
    /// deadlines. `None` means Radio can remain event-driven and asleep.
    fn next_radio_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }
    fn peer_activity(&self) -> Vec<PeerActivityEvidence> {
        PeerSessionPort::peer_activity(self)
    }
    /// Cumulative durable writes reported by worker-owned stores. The runtime
    /// samples this counter and records only the delta in its ledger.
    fn database_write_count(&self) -> u64 {
        0
    }

    fn blob_write_count(&self) -> u64 {
        0
    }

    fn attachment_chunk_tx_count(&self) -> u64 {
        0
    }

    fn attachment_policy_suppressed_count(&self) -> u64 {
        0
    }

    /// Contacts with durable control outbox work (receipts, reactions or
    /// attachment controls). This must not enumerate the contact book.
    fn active_control_contacts(&self) -> Vec<ContactId> {
        Vec::new()
    }

    fn queue_reaction(
        &mut self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
}
/// Provider-neutral lifecycle owned by the selected communication stack.
///
/// Each provider maps its own commissioning and reachability machinery to
/// this lifecycle.
pub trait CommunicationLifecycle: Send + 'static {
    /// Identity of the provider which owns this lifecycle.  Requiring this
    /// instead of defaulting commissioning to Tor prevents a newly added
    /// provider from accidentally exposing a Tor-shaped runtime snapshot.
    fn provider_id(&self) -> torca_foundation::ProviderId;
    /// Optional provider-owned deployment profile for diagnostics. This is
    /// deliberately presentation-only; runtime policy must not branch on a
    /// concrete provider string.
    fn provider_profile(&self) -> Option<&'static str> {
        None
    }
    /// Provider-owned grace period used when the host leaves the foreground.
    /// This is deliberately a policy hint, not a polling interval: the
    /// runtime schedules at most one transition to idle and otherwise waits
    /// for events. Low-cost direct Iroh profiles can release UI-only work
    /// sooner than relay-backed providers without a provider-name branch in
    /// RuntimeOwner.
    fn background_grace(&self) -> Duration {
        Duration::from_secs(30)
    }
    /// Provider-neutral, redaction-safe facts for diagnostics. Providers may
    /// expose endpoint/network generations and reachability state without
    /// leaking addresses or making the runtime depend on their library API.
    fn runtime_diagnostics(&self) -> torca_transport_api::ProviderRuntimeDiagnostics {
        torca_transport_api::ProviderRuntimeDiagnostics::default()
    }
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    /// Notify the provider that the platform network generation changed.
    /// Providers may migrate their endpoint or invalidate stale reachability
    /// evidence. Transports that follow platform changes automatically can
    /// keep the default no-op implementation.
    fn network_changed(&mut self, _now: Timestamp) {}
    /// Requests an explicit provider-owned route refresh. RuntimeOwner does
    /// not know whether this means QUIC migration, ICE renegotiation or a
    /// Tor endpoint rebuild; providers own that implementation detail.
    fn refresh_route(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        self.network_changed(now);
        Ok(())
    }
    /// Returns the next provider lifecycle deadline. A healthy, reachable and
    /// idle provider has no application-owned deadline and waits for a command
    /// or explicit network event instead of being polled.
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }
    /// Installs a non-blocking wake path for provider lifecycle events.
    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}
    /// Requests platform-neutral background activity policy.
    fn set_dormant(&mut self, _dormant: bool) -> Result<(), RuntimeDriverError> {
        Ok(())
    }
    /// Tells the provider whether incoming reachability is currently a real
    /// runtime demand. Providers with a public rendezvous service may use
    /// this to defer discovery/relay probes while the app is idle; direct
    /// providers can keep the default no-op implementation.
    fn set_reachability_demand(&mut self, _demanded: bool) {}
    fn state(&self) -> CommunicationState;
    fn local_endpoint_summary(&self) -> Option<String>;
    fn incoming_reachability_state(&self) -> IncomingReachabilityState {
        if self.local_endpoint_summary().is_some() {
            IncomingReachabilityState::Publishing
        } else {
            IncomingReachabilityState::Unknown
        }
    }
    /// Provider-owned readiness projection. Runtime and UI consume this
    /// neutral snapshot instead of inferring implementation details from a
    /// provider's endpoint string.
    fn commissioning(&self) -> torca_transport_api::ProviderCommissioning {
        use torca_transport_api::{
            CommissioningStage, CommissioningState, CommissioningStep, ProviderCommissioning,
        };

        let runtime = match self.state() {
            CommunicationState::Ready => CommissioningState::Ready,
            CommunicationState::Starting => CommissioningState::Pending,
            CommunicationState::Stopped => CommissioningState::Pending,
            CommunicationState::Degraded => CommissioningState::Degraded,
            CommunicationState::Failed => CommissioningState::Failed,
        };
        let incoming = match self.incoming_reachability_state() {
            IncomingReachabilityState::Reachable => CommissioningState::Ready,
            IncomingReachabilityState::Publishing | IncomingReachabilityState::Unknown => {
                CommissioningState::Pending
            }
            IncomingReachabilityState::Degraded => CommissioningState::Degraded,
            IncomingReachabilityState::Failed => CommissioningState::Failed,
            IncomingReachabilityState::Stopped => CommissioningState::NotRequired,
        };
        ProviderCommissioning {
            provider: self.provider_id(),
            steps: vec![
                CommissioningStep {
                    stage: CommissioningStage::LocalRuntime,
                    state: runtime,
                    required_for_local_shell: true,
                    required_for_pairing: true,
                },
                CommissioningStep {
                    stage: CommissioningStage::IncomingReachability,
                    state: incoming,
                    required_for_local_shell: false,
                    required_for_pairing: true,
                },
            ],
            endpoint_summary: self.local_endpoint_summary(),
            route_state: if self.local_endpoint_summary().is_some() {
                torca_transport_api::ProviderRouteState::Fresh
            } else {
                torca_transport_api::ProviderRouteState::Unavailable
            },
            pairing_bootstrap: None,
        }
    }
    fn shutdown(&mut self);
}

impl CommunicationLifecycle for Box<dyn CommunicationLifecycle> {
    fn provider_id(&self) -> torca_foundation::ProviderId {
        (**self).provider_id()
    }

    fn provider_profile(&self) -> Option<&'static str> {
        (**self).provider_profile()
    }

    fn background_grace(&self) -> Duration {
        (**self).background_grace()
    }

    fn runtime_diagnostics(&self) -> torca_transport_api::ProviderRuntimeDiagnostics {
        (**self).runtime_diagnostics()
    }

    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        (**self).maintenance(now)
    }

    fn network_changed(&mut self, now: Timestamp) {
        (**self).network_changed(now);
    }

    fn refresh_route(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        (**self).refresh_route(now)
    }

    fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        (**self).next_maintenance_delay(now)
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        (**self).set_waker(waker);
    }

    fn set_dormant(&mut self, dormant: bool) -> Result<(), RuntimeDriverError> {
        (**self).set_dormant(dormant)
    }

    fn set_reachability_demand(&mut self, demanded: bool) {
        (**self).set_reachability_demand(demanded);
    }

    fn state(&self) -> CommunicationState {
        (**self).state()
    }

    fn local_endpoint_summary(&self) -> Option<String> {
        (**self).local_endpoint_summary()
    }

    fn incoming_reachability_state(&self) -> IncomingReachabilityState {
        (**self).incoming_reachability_state()
    }

    fn commissioning(&self) -> torca_transport_api::ProviderCommissioning {
        (**self).commissioning()
    }

    fn shutdown(&mut self) {
        (**self).shutdown();
    }
}

/// Provider rendezvous connectivity is supervised outside the actor's critical path. A
/// probe implementation must be cheap to clone through `Arc` and may perform
/// blocking network work on the worker thread created by the supervisor.
pub trait RendezvousProbe: Send + Sync + 'static {
    fn probe(&self) -> Result<(), ErrorCode>;

    fn service_info(&self) -> Option<RendezvousServiceInfo> {
        None
    }
}

/// Compatibility alias for adapters compiled against the old port name.
pub use RendezvousProbe as RelayProbe;

struct RuntimePairingServiceHealthPort(Arc<dyn RendezvousProbe>);

impl PairingServiceHealthPort for RuntimePairingServiceHealthPort {
    fn check_relay_health(&self) -> Result<(), ErrorCode> {
        self.0.probe()
    }
}
