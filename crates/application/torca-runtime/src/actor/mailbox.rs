enum RuntimeCommand {
    SetAttention(AttentionContext),
    CreatePairing(PairingSessionId, Sender<Result<PairingInvitationView, RuntimeDriverError>>),
    JoinPairing(
        PairingSessionId,
        PairingCode,
        Option<[u8; 16]>,
        Sender<Result<(), RuntimeDriverError>>,
    ),
    ApprovePairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    RejectPairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    CancelPairing(PairingSessionId, Sender<Result<(), RuntimeDriverError>>),
    VerifyContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    ResetContactVerification(ContactId, Sender<Result<(), RuntimeDriverError>>),
    RenameContact(ContactId, String, Sender<Result<(), RuntimeDriverError>>),
    BlockContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    UnblockContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    RemoveContact(ContactId, Sender<Result<(), RuntimeDriverError>>),
    ClearConversationHistory(ConversationId, Sender<Result<(), RuntimeDriverError>>),
    MarkConversationRead(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    QueueAttachment(AttachmentSendRequest, Sender<Result<(), RuntimeDriverError>>),
    QueueReaction(ContactId, ReactionPayload, Timestamp, Sender<Result<(), RuntimeDriverError>>),
    RetryAttachment(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    CancelAttachment(OpaqueId, Sender<Result<(), RuntimeDriverError>>),
    ExportAttachment(AttachmentId, PathBuf, Sender<Result<(), RuntimeDriverError>>),
    ExportAttachmentPreview(AttachmentId, PathBuf, Sender<Result<(), RuntimeDriverError>>),
    AttachmentSnapshot(Sender<Result<Vec<AttachmentView>, RuntimeDriverError>>),
    NetworkSnapshot(Sender<Result<NetworkSnapshot, RuntimeDriverError>>),
    Diagnostics(Sender<String>),
    NetworkChanged,
    SetRadioDemand(ContactId, bool),
    SetInstantContactDemand(ContactId, bool),
    SetRadioTransmission(ContactId, bool),
    SetBatteryProfile(BatteryProfile),
    SetForeground(bool),
    SetMeteredNetwork(bool),
    SetMeteredTransferPolicy(MeteredTransferPolicy),
    SetTorDormancy(bool),
    Wake,
    WakeDelivery(OpaqueId),
    ReleaseDelivery(OpaqueId),
    Shutdown(Sender<()>),
}

#[derive(Clone)]
pub struct RuntimeHandle {
    sender: SyncSender<RuntimeCommand>,
}
impl RuntimeHandle {
    /// True while durable delivery, attachment transfer or radio work must
    /// keep the network awake. This is a lock-free observation used by the
    /// platform battery coordinator; the actor remains the sole writer.
    pub fn set_attention(&self, context: AttentionContext) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::SetAttention(context));
    }

    pub fn create_pairing(
        &self,
        id: PairingSessionId,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::CreatePairing(id, r))
    }
    pub fn join_pairing(
        &self,
        id: PairingSessionId,
        code: PairingCode,
    ) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::JoinPairing(id, code, None, r))
    }

    pub fn join_pairing_with_ticket(
        &self,
        id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
    ) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::JoinPairing(id, code, ticket, r))
    }
    pub fn approve_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::ApprovePairing(id, r))
    }
    pub fn reject_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::RejectPairing(id, r))
    }
    pub fn cancel_pairing(&self, id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::CancelPairing(id, r))
    }
    pub fn verify_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::VerifyContact(id, r))
    }
    pub fn reset_contact_verification(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::ResetContactVerification(id, r))
    }
    pub fn rename_contact(&self, id: ContactId, name: String) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::RenameContact(id, name, r))
    }
    pub fn block_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::BlockContact(id, r))
    }
    pub fn unblock_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::UnblockContact(id, r))
    }
    pub fn remove_contact(&self, id: ContactId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::RemoveContact(id, r))
    }
    pub fn clear_conversation_history(&self, id: ConversationId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::ClearConversationHistory(id, r))
    }
    pub fn mark_conversation_read(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::MarkConversationRead(id, r))
    }
    pub fn queue_attachment(&self, value: AttachmentSendRequest) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::QueueAttachment(value, r))
    }
    pub fn queue_reaction(
        &self,
        contact_id: ContactId,
        reaction: ReactionPayload,
        at: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| {
            RuntimeCommand::QueueReaction(contact_id, reaction, at, r)
        })
    }
    pub fn retry_attachment(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::RetryAttachment(id, r))
    }
    pub fn cancel_attachment(&self, id: OpaqueId) -> Result<(), RuntimeDriverError> {
        request_command(&self.sender, |r| RuntimeCommand::CancelAttachment(id, r))
    }
    pub fn export_attachment(
        &self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError> {
        request_blocking(&self.sender, |r| RuntimeCommand::ExportAttachment(id, destination, r))
    }
    pub fn export_attachment_preview(
        &self,
        id: AttachmentId,
        destination: PathBuf,
    ) -> Result<(), RuntimeDriverError> {
        request_blocking(&self.sender, |r| {
            RuntimeCommand::ExportAttachmentPreview(id, destination, r)
        })
    }
    pub fn attachment_snapshot(&self) -> Result<Vec<AttachmentView>, RuntimeDriverError> {
        request_query(&self.sender, RuntimeCommand::AttachmentSnapshot)
    }
    pub fn network_snapshot(&self) -> Result<NetworkSnapshot, RuntimeDriverError> {
        request_query(&self.sender, RuntimeCommand::NetworkSnapshot)
    }
    pub fn diagnostics_json(&self) -> Result<String, RuntimeDriverError> {
        let (tx, rx) = mpsc::channel();
        send_with_timeout(&self.sender, RuntimeCommand::Diagnostics(tx))?;
        rx.recv_timeout(QUERY_WAIT).map_err(|_| RuntimeDriverError::Communication)
    }
    pub fn wake_delivery(&self) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::Wake);
    }

    /// Wakes durable delivery and grants a temporary lease for this message.
    /// The lease is independent of the currently visible Flutter route.
    pub fn wake_delivery_for(&self, message_id: OpaqueId) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::WakeDelivery(message_id));
    }

    pub fn release_delivery(&self, message_id: OpaqueId) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::ReleaseDelivery(message_id));
    }

    /// Notify the actor that the platform network changed. This resets the
    /// relay supervisor immediately instead of waiting for its backoff timer.
    pub fn network_changed(&self) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::NetworkChanged);
    }

    /// Keeps the selected radio peer leased independently of the Flutter route.
    pub fn set_radio_demand(&self, contact_id: ContactId, enabled: bool) {
        let _ =
            send_with_timeout(&self.sender, RuntimeCommand::SetRadioDemand(contact_id, enabled));
    }

    /// Keeps a user-selected contact immediately reachable until explicitly
    /// disabled. The setting owner persists intent; this actor owns only the
    /// process-local connection lease.
    pub fn set_instant_contact_demand(&self, contact_id: ContactId, enabled: bool) {
        let _ = send_with_timeout(
            &self.sender,
            RuntimeCommand::SetInstantContactDemand(contact_id, enabled),
        );
    }

    /// Keeps a short lease while a push-to-talk transmission is being negotiated.
    pub fn set_radio_transmission(&self, contact_id: ContactId, active: bool) {
        let _ = send_with_timeout(
            &self.sender,
            RuntimeCommand::SetRadioTransmission(contact_id, active),
        );
    }

    /// Selects the runtime battery profile. This changes discretionary work
    /// policy and diagnostics; durable delivery remains unaffected.
    pub fn set_battery_profile(&self, profile: BatteryProfile) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::SetBatteryProfile(profile));
    }

    pub fn set_metered_network(&self, metered: bool) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::SetMeteredNetwork(metered));
    }

    pub fn set_foreground(&self, foreground: bool) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::SetForeground(foreground));
    }

    pub fn set_metered_transfer_policy(&self, policy: MeteredTransferPolicy) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::SetMeteredTransferPolicy(policy));
    }

    pub fn set_tor_dormancy(&self, dormant: bool) {
        let _ = send_with_timeout(&self.sender, RuntimeCommand::SetTorDormancy(dormant));
    }
}

pub struct RuntimeOwner {
    sender: SyncSender<RuntimeCommand>,
    join: Option<JoinHandle<()>>,
    relay_worker: Option<RelayHealthWorker>,
}

enum RuntimeWait {
    Command(RuntimeCommand),
    Timeout,
    Closed,
}

fn wait_for_runtime_command(
    receiver: &Receiver<RuntimeCommand>,
    deadline: Option<std::time::Instant>,
) -> RuntimeWait {
    match deadline {
        Some(deadline) => match receiver
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
        {
            Ok(command) => RuntimeWait::Command(command),
            Err(RecvTimeoutError::Timeout) => RuntimeWait::Timeout,
            Err(RecvTimeoutError::Disconnected) => RuntimeWait::Closed,
        },
        None => match receiver.recv() {
            Ok(command) => RuntimeWait::Command(command),
            Err(_) => RuntimeWait::Closed,
        },
    }
}

fn command_writes_database(command: &RuntimeCommand) -> bool {
    matches!(
        command,
        RuntimeCommand::CreatePairing(..)
            | RuntimeCommand::JoinPairing(..)
            | RuntimeCommand::ApprovePairing(..)
            | RuntimeCommand::RejectPairing(..)
            | RuntimeCommand::CancelPairing(..)
            | RuntimeCommand::VerifyContact(..)
            | RuntimeCommand::ResetContactVerification(..)
            | RuntimeCommand::RenameContact(..)
            | RuntimeCommand::BlockContact(..)
            | RuntimeCommand::UnblockContact(..)
            | RuntimeCommand::RemoveContact(..)
            | RuntimeCommand::ClearConversationHistory(..)
            | RuntimeCommand::MarkConversationRead(..)
            | RuntimeCommand::QueueAttachment(..)
            | RuntimeCommand::RetryAttachment(..)
            | RuntimeCommand::CancelAttachment(..)
    )
}

fn command_requires_network(command: &RuntimeCommand) -> bool {
    matches!(
        command,
        RuntimeCommand::CreatePairing(..)
            | RuntimeCommand::JoinPairing(..)
            | RuntimeCommand::ApprovePairing(..)
            | RuntimeCommand::RejectPairing(..)
            | RuntimeCommand::CancelPairing(..)
            | RuntimeCommand::MarkConversationRead(..)
            | RuntimeCommand::QueueAttachment(..)
            | RuntimeCommand::QueueReaction(..)
            | RuntimeCommand::RetryAttachment(..)
            | RuntimeCommand::CancelAttachment(..)
    )
}
