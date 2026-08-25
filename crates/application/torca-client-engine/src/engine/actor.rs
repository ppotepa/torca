// Responsibility: bounded single-writer engine mailbox and actor lifetime.

enum ActorRequest {
    Dispatch(Box<EngineCommand>, Sender<Result<EngineResult, EngineError>>),
    Snapshot(Sender<Result<ClientSnapshot, EngineError>>),
    OverviewSnapshot(Sender<Result<ClientSnapshot, EngineError>>),
    AvatarGenomeForIdentity(IdentityId, Sender<Result<Option<AvatarGenomeRecord>, EngineError>>),
    MessageStatus(MessageId, Sender<Result<Option<MessageStatus>, EngineError>>),
    Message(MessageId, Sender<Result<Option<Message>, EngineError>>),
    MessageContact(MessageId, Sender<Result<Option<ContactId>, EngineError>>),
    PendingDeliveryContacts(Sender<Result<Vec<ContactId>, EngineError>>),
    Shutdown,
}

#[derive(Clone)]
pub struct EngineHandle {
    sender: SyncSender<ActorRequest>,
    projection_events: Arc<AtomicU64>,
}
impl EngineHandle {
    pub fn dispatch(&self, command: EngineCommand) -> Result<EngineResult, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::Dispatch(Box::new(command), sender))?;
        receiver.recv_timeout(Duration::from_secs(10)).map_err(|_| EngineError::Unavailable)?
    }
    pub fn snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::Snapshot(sender))?;
        receiver.recv_timeout(Duration::from_secs(5)).map_err(|_| EngineError::Unavailable)?
    }
    pub fn overview_snapshot(&self) -> Result<ClientSnapshot, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::OverviewSnapshot(sender))?;
        receiver.recv_timeout(Duration::from_secs(5)).map_err(|_| EngineError::Unavailable)?
    }
    pub fn avatar_genome_for_identity(
        &self,
        identity_id: IdentityId,
    ) -> Result<Option<AvatarGenomeRecord>, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(
            &self.sender,
            ActorRequest::AvatarGenomeForIdentity(identity_id, sender),
        )?;
        receiver.recv_timeout(Duration::from_secs(5)).map_err(|_| EngineError::Unavailable)?
    }
    pub fn message_status(
        &self,
        message_id: MessageId,
    ) -> Result<Option<MessageStatus>, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::MessageStatus(message_id, sender))?;
        receiver.recv_timeout(Duration::from_secs(5)).map_err(|_| EngineError::Unavailable)?
    }

    pub fn message(&self, message_id: MessageId) -> Result<Option<Message>, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::Message(message_id, sender))?;
        receiver.recv_timeout(Duration::from_secs(5)).map_err(|_| EngineError::Unavailable)?
    }
    pub fn message_contact(
        &self,
        message_id: MessageId,
    ) -> Result<Option<ContactId>, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::MessageContact(message_id, sender))?;
        receiver.recv_timeout(Duration::from_secs(5)).map_err(|_| EngineError::Unavailable)?
    }
    pub fn pending_delivery_contacts(&self) -> Result<Vec<ContactId>, EngineError> {
        let (sender, receiver) = mpsc::channel();
        send_with_timeout(&self.sender, ActorRequest::PendingDeliveryContacts(sender))?;
        receiver.recv_timeout(Duration::from_secs(5)).map_err(|_| EngineError::Unavailable)?
    }
    pub fn projection_event_count(&self) -> u64 {
        self.projection_events.load(Ordering::Acquire)
    }
}

pub struct ClientEngineActor {
    sender: SyncSender<ActorRequest>,
    join: Option<JoinHandle<()>>,
}
impl ClientEngineActor {
    /// Starts the single-threaded engine actor and returns its command handle.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot create the named actor thread.
    pub fn spawn<E: EngineRuntime>(mut engine: E) -> (EngineHandle, Self) {
        let (sender, receiver): (SyncSender<ActorRequest>, Receiver<ActorRequest>) =
            mpsc::sync_channel(256);
        let projection_events = Arc::new(AtomicU64::new(0));
        let handle = EngineHandle {
            sender: sender.clone(),
            projection_events: Arc::clone(&projection_events),
        };
        let join = thread::Builder::new()
            .name("torca-client-engine".into())
            .spawn(move || {
                let mut activity = EngineActivityGuard::default();
                loop {
                    let request = match receiver.recv() {
                        Ok(request) => request,
                        Err(_) => break,
                    };
                    activity.observe();
                    match request {
                        ActorRequest::Dispatch(command, response) => {
                            let counts_projection = matches!(
                                &*command,
                                EngineCommand::ApplyReceipt(_)
                                    | EngineCommand::SetMessageReaction { .. }
                            );
                            let result = engine.dispatch(*command);
                            if counts_projection && result.is_ok() {
                                projection_events.fetch_add(1, Ordering::Release);
                            }
                            let _ = response.send(result);
                        }
                        ActorRequest::Snapshot(response) => {
                            let _ = response.send(engine.snapshot());
                        }
                        ActorRequest::OverviewSnapshot(response) => {
                            let _ = response.send(engine.overview_snapshot());
                        }
                        ActorRequest::AvatarGenomeForIdentity(identity_id, response) => {
                            let _ = response.send(engine.avatar_genome_for_identity(identity_id));
                        }
                        ActorRequest::MessageStatus(message_id, response) => {
                            let _ = response.send(engine.message_status(message_id));
                        }
                        ActorRequest::Message(message_id, response) => {
                            let _ = response.send(engine.message(message_id));
                        }
                        ActorRequest::MessageContact(message_id, response) => {
                            let _ = response.send(engine.message_contact(message_id));
                        }
                        ActorRequest::PendingDeliveryContacts(response) => {
                            let _ = response.send(engine.pending_delivery_contacts());
                        }
                        ActorRequest::Shutdown => break,
                    }
                }
            })
            .expect("spawn torca client engine actor");
        (handle, Self { sender, join: Some(join) })
    }

    pub fn shutdown(mut self) -> Result<(), EngineError> {
        send_with_timeout(&self.sender, ActorRequest::Shutdown)?;
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| EngineError::Unavailable)?;
        }
        Ok(())
    }
}

/// Emits a bounded diagnostic when the engine mailbox is being driven at an
/// unexpectedly high rate.  This is intentionally local to the actor and does
/// not add a timer or retain any request payloads.
#[derive(Default)]
struct EngineActivityGuard {
    window_started: Option<std::time::Instant>,
    requests: u32,
}

impl EngineActivityGuard {
    fn observe(&mut self) {
        let now = std::time::Instant::now();
        let started = *self.window_started.get_or_insert(now);
        self.requests = self.requests.saturating_add(1);
        if now.duration_since(started) < std::time::Duration::from_secs(1) {
            return;
        }
        if self.requests > 100 {
            eprintln!(
                "torca-client-engine: high mailbox activity requests_per_second={}",
                self.requests
            );
        }
        self.window_started = Some(now);
        self.requests = 0;
    }
}

fn send_with_timeout(
    sender: &SyncSender<ActorRequest>,
    mut request: ActorRequest,
) -> Result<(), EngineError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match sender.try_send(request) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => return Err(EngineError::Unavailable),
            Err(TrySendError::Full(returned)) => {
                if Instant::now() >= deadline {
                    return Err(EngineError::Unavailable);
                }
                request = returned;
                thread::yield_now();
            }
        }
    }
}
