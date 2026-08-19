static REGISTRY: OnceLock<Mutex<Option<Arc<RuntimeHandleInner>>>> = OnceLock::new();
static METADATA: OnceLock<Vec<u8>> = OnceLock::new();
static INITIALIZATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

enum ActorMessage {
    Invoke {
        request: String,
        response: SyncSender<Vec<u8>>,
    },
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    Lifecycle {
        event: String,
        response: SyncSender<i32>,
    },
    Shutdown {
        response: SyncSender<()>,
    },
}

struct RuntimeHandleInner {
    sender: SyncSender<ActorMessage>,
    startup_error: Option<String>,
    event_hub: Arc<RuntimeEventHub>,
    alive: Arc<AtomicBool>,
}

#[repr(C)]
pub struct TorcaRuntimeHandle {
    inner: Arc<RuntimeHandleInner>,
    response: Mutex<Vec<u8>>,
}

struct ActorState {
    runtime: TorcaRuntime,
    runtime_id: String,
    revision: u64,
    completed: IdempotencyLedger,
}

struct CompletedCommand {
    response: Vec<u8>,
    completed_at: Instant,
}

struct IdempotencyLedger {
    entries: HashMap<String, CompletedCommand>,
    order: VecDeque<String>,
    max_entries: usize,
    ttl: Duration,
}

impl Default for IdempotencyLedger {
    fn default() -> Self {
        Self::with_limits(IDEMPOTENCY_MAX_ENTRIES, IDEMPOTENCY_TTL)
    }
}

impl IdempotencyLedger {
    fn with_limits(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
            ttl,
        }
    }

    fn prune(&mut self, now: Instant) {
        while let Some(request_id) = self.order.front().cloned() {
            let expired = self
                .entries
                .get(&request_id)
                .is_none_or(|entry| now.duration_since(entry.completed_at) >= self.ttl);
            if !expired && self.entries.len() <= self.max_entries {
                break;
            }
            self.order.pop_front();
            self.entries.remove(&request_id);
        }
    }

    fn get(&mut self, request_id: &str, now: Instant) -> Option<Vec<u8>> {
        self.prune(now);
        self.entries.get(request_id).map(|entry| entry.response.clone())
    }

    fn insert(&mut self, request_id: String, response: Vec<u8>, now: Instant) {
        self.prune(now);
        if self.entries.contains_key(&request_id) {
            self.order.retain(|value| value != &request_id);
        }
        self.entries.insert(request_id.clone(), CompletedCommand { response, completed_at: now });
        self.order.push_back(request_id);
        self.prune(now);
    }
}
