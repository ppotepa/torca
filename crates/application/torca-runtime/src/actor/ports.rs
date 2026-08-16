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
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn approve(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError>;
    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError>;
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    /// Returns the next useful maintenance deadline. `None` means the worker
    /// can sleep until a command or network event arrives; it must not wake
    /// just to discover that there is no pairing work.
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }
    fn network_changed(&mut self, _now: Timestamp) {}
    fn shutdown(&mut self);
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

    fn queue_reaction(
        &mut self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), RuntimeDriverError>;
}
pub trait TorDriver: Send + 'static {
    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError>;
    /// Returns the next Tor lifecycle deadline. A healthy, reachable and
    /// idle Tor service has no application-owned deadline and must wait for a
    /// command or an explicit network event instead of being polled.
    fn next_maintenance_delay(&self, _now: Timestamp) -> Option<Duration> {
        None
    }
    /// Installs a non-blocking wake path for Tor/bootstrap/publisher events.
    fn set_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}
    /// Requests platform-neutral Tor background activity policy. The default
    /// keeps test drivers compatible; production Arti drivers implement it.
    fn set_dormant(&mut self, _dormant: bool) -> Result<(), RuntimeDriverError> {
        Ok(())
    }
    fn state(&self) -> TorState;
    fn onion_address(&self) -> Option<String>;
    fn onion_service_state(&self) -> OnionServiceState {
        if self.onion_address().is_some() {
            OnionServiceState::Publishing
        } else {
            OnionServiceState::Unknown
        }
    }
    fn shutdown(&mut self);
}

/// Relay connectivity is supervised outside the actor's critical path. A
/// probe implementation must be cheap to clone through `Arc` and may perform
/// blocking network work on the worker thread created by the supervisor.
pub trait RelayProbe: Send + Sync + 'static {
    fn probe(&self) -> Result<(), ErrorCode>;

    fn service_info(&self) -> Option<RelayServiceInfo> {
        None
    }
}

struct RuntimeRelayHealthPort(Arc<dyn RelayProbe>);

impl RelayHealthPort for RuntimeRelayHealthPort {
    fn check_relay_health(&self) -> Result<(), ErrorCode> {
        self.0.probe()
    }
}
