use std::collections::{BTreeSet, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand_core::{OsRng, RngCore};
use torca_contacts::ContactId;
use torca_crypto::{Ciphertext, CryptoProvider, Nonce, RadioSessionCipher};
use torca_foundation::{OpaqueId, Timestamp};
use torca_radio::{MAX_RADIO_BURST_MS, RadioOperationId, RadioSessionId};
use torca_radio_coordinator::{
    RadioApplicationError, RadioMediaPort, RadioSessionEvent, RadioTransportFailure,
};
use torca_radio_protocol::{
    BurstEndReason, FloorDeniedReason, MAX_RADIO_BURST_FRAMES, MAX_RADIO_MEDIA_FRAME,
    RADIO_MEDIA_PROOF_LEN, RADIO_PROTOCOL_VERSION, RadioMediaCodec, RadioMediaFrame,
    SessionCloseReason,
};
use torca_tor::{PeerListener, TOR_RADIO_VIRTUAL_PORT, TorServiceHandle};
use torca_transport_api::RealtimeCapabilities;

use crate::{AudioPipeline, JitterBuffer};

const COMMAND_CAPACITY: usize = 32;
const EVENT_CAPACITY: usize = 64;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const RECONNECT_BASE_DELAY: Duration = Duration::from_millis(500);
const RECONNECT_MAX_DELAY: Duration = Duration::from_secs(8);
const ACTIVE_READ_TIMEOUT: Duration = Duration::from_millis(20);
// Keep the authenticated media stream alive without waking the device every
// few seconds. The 10-second interval remains comfortably below the
// connection idle budget, while halving idle radio traffic during a session.
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(30);
const FLOOR_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
// This is a safety ceiling for providers which do not expose a more precise
// transport idle budget.  It must never be used to back off past the
// provider's advertised interval: QUIC/TURN/onion implementations may close
// an otherwise healthy realtime lane while it is idle.
const KEEP_ALIVE_MAX_INTERVAL: Duration = Duration::from_secs(120);
const AUDIO_RETRANSMIT_AFTER: Duration = Duration::from_millis(250);
const AUDIO_FRAME_INTERVAL: Duration = Duration::from_millis(20);
const MAX_RETRANSMITS_PER_TICK: usize = 8;
const MAX_UNACKED_AUDIO_AGE: Duration = Duration::from_secs(8);
const CONNECTION_IDLE_LIMIT: Duration = Duration::from_secs(180);
const READ_BUFFER_LIMIT: usize = (MAX_RADIO_MEDIA_FRAME + 4) * 8;
const COMPLETED_BURST_HISTORY: usize = 8;

/// Onion and identity data needed by the media worker for one accepted
/// contact. The directory remains the owner of relationship secrets.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadioMediaRoute {
    /// Provider-owned route bytes. The common media protocol never assumes
    /// that a route is an onion address; provider factories decode these
    /// bytes according to their selected transport.
    pub provider: String,
    pub endpoint: Vec<u8>,
    pub local_identity: OpaqueId,
    pub remote_identity: OpaqueId,
}

/// Blocking byte stream used by the bounded radio worker. Providers may
/// implement it over TCP, QUIC or a platform DataChannel; the coordinator
/// never sees the concrete socket type.
pub trait RadioMediaStream: Read + Write + Send {
    fn configure(&self, _read: Duration, _write: Duration) -> Result<(), RadioApplicationError> {
        Ok(())
    }

    fn set_read_deadline(&self, _timeout: Duration) -> Result<(), RadioApplicationError> {
        Ok(())
    }

    fn close_stream(&self) -> Result<(), RadioApplicationError>;
}

/// Provider-owned media connector. It is deliberately synchronous at the
/// worker boundary so audio cadence and retransmit deadlines stay bounded;
/// provider implementations may block on their own async runtime internally.
pub trait RadioMediaConnector: Send {
    /// Transport facts shared by all communication providers.  The media
    /// worker uses the idle budget to schedule heartbeats; product semantics
    /// remain in `RadioCoordinator`.
    fn capabilities(&self) -> RealtimeCapabilities {
        RealtimeCapabilities::default()
    }

    fn connect(
        &mut self,
        route: &RadioMediaRoute,
        timeout: Duration,
    ) -> Result<Box<dyn RadioMediaStream>, RadioApplicationError>;

    fn try_accept(&mut self) -> Result<Option<Box<dyn RadioMediaStream>>, RadioApplicationError>;

    /// Provider-specific idle keep-alive.  QUIC providers already have an
    /// internal heartbeat and need a shorter application deadline than the
    /// legacy stream transports, otherwise the provider can close an idle
    /// media stream before the next application frame is sent.
    fn keep_alive_interval(&self) -> Duration {
        Duration::from_millis(self.capabilities().max_idle_interval_ms).min(KEEP_ALIVE_INTERVAL)
    }
}

/// Erased session cipher used by the socket worker. Implementations retain
/// only ephemeral directional keys, never the durable pairwise secret.
pub trait RadioMediaCipher: Send {
    fn seal(
        &self,
        nonce: [u8; 24],
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, RadioApplicationError>;

    fn open(
        &self,
        nonce: [u8; 24],
        associated_data: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, RadioApplicationError>;
}

impl<C> RadioMediaCipher for RadioSessionCipher<C>
where
    C: CryptoProvider + Send,
{
    fn seal(
        &self,
        nonce: [u8; 24],
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, RadioApplicationError> {
        RadioSessionCipher::seal(self, Nonce(nonce), associated_data, plaintext)
            .map(|ciphertext| ciphertext.0)
            .map_err(|_| RadioApplicationError::Crypto)
    }

    fn open(
        &self,
        nonce: [u8; 24],
        associated_data: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, RadioApplicationError> {
        RadioSessionCipher::open(
            self,
            Nonce(nonce),
            associated_data,
            &Ciphertext(ciphertext.to_vec()),
        )
        .map_err(|_| RadioApplicationError::Crypto)
    }
}

/// Relationship-backed media lookup. The factory is called for every new
/// connection attempt so a failed socket never retains stale key state.
pub trait RadioMediaDirectory: Send {
    fn route(&self, contact_id: ContactId) -> Option<RadioMediaRoute>;

    fn session_cipher(
        &self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        media_token: &[u8; 32],
    ) -> Result<Box<dyn RadioMediaCipher>, RadioApplicationError>;
}

/// Provider-owned constructor for one radio media system.
///
/// The communication composition depends only on this boundary. A provider
/// decides how media sessions are reached; it must not leak a Tor client,
/// onion listener, WebRTC channel or Iroh endpoint into common delivery code.
pub trait RadioMediaSystemFactory: Send {
    fn start(
        self: Box<Self>,
        directory: Box<dyn RadioMediaDirectory>,
    ) -> Result<RadioMediaSystem, RadioApplicationError>;
}

/// Tor-specific factory kept beside the existing TCP/onion media worker.
/// Future providers implement `RadioMediaSystemFactory` in their own
/// composition module without changing communication assembly.
pub struct TorRadioMediaSystemFactory {
    tor: TorServiceHandle,
}

struct TorRadioMediaStream(TcpStream);

impl Read for TorRadioMediaStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buffer)
    }
}
impl Write for TorRadioMediaStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
impl RadioMediaStream for TorRadioMediaStream {
    fn configure(&self, read: Duration, write: Duration) -> Result<(), RadioApplicationError> {
        self.0
            .set_read_timeout(Some(read))
            .and_then(|_| self.0.set_write_timeout(Some(write)))
            .and_then(|_| self.0.set_nodelay(true))
            .map_err(|_| RadioApplicationError::MediaTransport)
    }

    fn close_stream(&self) -> Result<(), RadioApplicationError> {
        self.0.shutdown(Shutdown::Both).map_err(|_| RadioApplicationError::MediaTransport)
    }

    fn set_read_deadline(&self, timeout: Duration) -> Result<(), RadioApplicationError> {
        self.0.set_read_timeout(Some(timeout)).map_err(|_| RadioApplicationError::MediaTransport)
    }
}

struct TorRadioMediaConnector {
    tor: TorServiceHandle,
    listener: PeerListener,
}

impl RadioMediaConnector for TorRadioMediaConnector {
    fn connect(
        &mut self,
        route: &RadioMediaRoute,
        timeout: Duration,
    ) -> Result<Box<dyn RadioMediaStream>, RadioApplicationError> {
        if route.provider != "tor" {
            return Err(RadioApplicationError::MediaTransport);
        }
        let onion = std::str::from_utf8(&route.endpoint)
            .map_err(|_| RadioApplicationError::MediaTransport)?;
        self.tor
            .connect_onion_with_timeout(onion, TOR_RADIO_VIRTUAL_PORT, timeout)
            .map(|stream| Box::new(TorRadioMediaStream(stream)) as Box<dyn RadioMediaStream>)
            .map_err(|_| RadioApplicationError::MediaTransport)
    }

    fn try_accept(&mut self) -> Result<Option<Box<dyn RadioMediaStream>>, RadioApplicationError> {
        let stream =
            self.listener.try_accept().map_err(|_| RadioApplicationError::MediaTransport)?.map(
                |(stream, _)| Box::new(TorRadioMediaStream(stream)) as Box<dyn RadioMediaStream>,
            );
        Ok(stream)
    }
}

impl TorRadioMediaSystemFactory {
    #[must_use]
    pub const fn new(tor: TorServiceHandle) -> Self {
        Self { tor }
    }
}

impl RadioMediaSystemFactory for TorRadioMediaSystemFactory {
    fn start(
        self: Box<Self>,
        directory: Box<dyn RadioMediaDirectory>,
    ) -> Result<RadioMediaSystem, RadioApplicationError> {
        RadioMediaSystem::start_tor(self.tor, directory)
    }
}

/// Explicit capability boundary for providers without a media route.
pub struct UnsupportedRadioMediaSystemFactory {
    provider: torca_transport_api::TransportKind,
}

impl UnsupportedRadioMediaSystemFactory {
    pub const fn new(provider: torca_transport_api::TransportKind) -> Self {
        Self { provider }
    }
}

impl RadioMediaSystemFactory for UnsupportedRadioMediaSystemFactory {
    fn start(
        self: Box<Self>,
        _directory: Box<dyn RadioMediaDirectory>,
    ) -> Result<RadioMediaSystem, RadioApplicationError> {
        let _ = self.provider;
        Ok(RadioMediaSystem::disabled())
    }
}

/// Both application-facing adapters created around one bounded media worker.
pub struct RadioMediaSystem {
    pub media: RadioMediaAdapter,
    pub audio: crate::RadioAudioAdapter,
}

impl RadioMediaSystem {
    /// Provider-neutral disabled media endpoint used when a transport does
    /// not expose a Radio route yet. Messaging startup remains independent of
    /// optional media capability.
    pub fn disabled() -> Self {
        Self { media: RadioMediaAdapter::disabled(), audio: crate::RadioAudioAdapter::disabled() }
    }

    pub fn start_tor(
        tor: TorServiceHandle,
        directory: Box<dyn RadioMediaDirectory>,
    ) -> Result<Self, RadioApplicationError> {
        let listener = PeerListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .map_err(|_| RadioApplicationError::MediaTransport)?;
        tor.register_onion_route(TOR_RADIO_VIRTUAL_PORT, listener.local_addr())
            .map_err(|_| RadioApplicationError::MediaTransport)?;
        let pipeline = AudioPipeline::default();
        #[cfg(target_os = "android")]
        crate::install_android_pipeline(pipeline.clone());
        let audio = crate::RadioAudioAdapter::new(pipeline.clone());
        let connector = TorRadioMediaConnector { tor, listener };
        let media = RadioMediaAdapter::start(Box::new(connector), directory, pipeline)?;
        Ok(Self { media, audio })
    }

    /// Starts media with a provider-owned connector. This is the extension
    /// point used by direct transports such as Iroh; common framing, crypto,
    /// jitter and audio queues remain shared.
    pub fn start_with_connector(
        connector: Box<dyn RadioMediaConnector>,
        directory: Box<dyn RadioMediaDirectory>,
    ) -> Result<Self, RadioApplicationError> {
        let pipeline = AudioPipeline::default();
        #[cfg(target_os = "android")]
        crate::install_android_pipeline(pipeline.clone());
        let audio = crate::RadioAudioAdapter::new(pipeline.clone());
        let media = RadioMediaAdapter::start(connector, directory, pipeline)?;
        Ok(Self { media, audio })
    }
}

pub struct RadioMediaAdapter {
    commands: SyncSender<MediaCommand>,
    events: Receiver<RadioSessionEvent>,
    wakeups: Arc<AtomicU64>,
    worker_alive: Arc<std::sync::atomic::AtomicBool>,
    waker: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
}

impl RadioMediaAdapter {
    pub fn disabled() -> Self {
        let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_CAPACITY);
        drop(command_rx);
        let (_event_tx, event_rx) = mpsc::sync_channel(EVENT_CAPACITY);
        Self {
            commands: command_tx,
            events: event_rx,
            wakeups: Arc::new(AtomicU64::new(0)),
            worker_alive: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            waker: Arc::new(Mutex::new(None)),
        }
    }

    fn start(
        connector: Box<dyn RadioMediaConnector>,
        directory: Box<dyn RadioMediaDirectory>,
        audio: AudioPipeline,
    ) -> Result<Self, RadioApplicationError> {
        let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (event_tx, event_rx) = mpsc::sync_channel(EVENT_CAPACITY);
        let wakeups = Arc::new(AtomicU64::new(0));
        let worker_alive = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let waker = Arc::new(Mutex::new(None));
        let worker_alive_flag = Arc::clone(&worker_alive);
        let worker_waker = Arc::clone(&waker);
        thread::Builder::new()
            .name("torca-radio-media".into())
            .spawn(move || {
                let worker = MediaWorker::new(
                    connector,
                    directory,
                    audio,
                    command_rx,
                    event_tx,
                    Arc::clone(&worker_alive_flag),
                    worker_waker,
                );
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.run()));
                if result.is_err() {
                    eprintln!("torca-radio: media worker panicked; marking worker unavailable");
                    worker_alive_flag.store(false, Ordering::Release);
                }
            })
            .map_err(|_| RadioApplicationError::MediaTransport)?;
        Ok(Self { commands: command_tx, events: event_rx, wakeups, worker_alive, waker })
    }

    fn submit(&self, command: MediaCommand) -> Result<(), RadioApplicationError> {
        self.commands.try_send(command).map_err(|error| {
            let (reason, failure) = match error {
                mpsc::TrySendError::Full(_) => {
                    ("command_queue_full", RadioApplicationError::MediaQueueFull)
                }
                mpsc::TrySendError::Disconnected(_) => {
                    ("worker_disconnected", RadioApplicationError::MediaWorkerUnavailable)
                }
            };
            eprintln!(
                "torca-radio: media command rejected reason={reason} worker_alive={}",
                self.worker_alive.load(Ordering::Acquire)
            );
            failure
        })
    }
}

impl Drop for RadioMediaAdapter {
    fn drop(&mut self) {
        let _ = self.commands.try_send(MediaCommand::Shutdown);
    }
}

impl RadioMediaPort for RadioMediaAdapter {
    fn wake_count(&self) -> u64 {
        self.wakeups.load(Ordering::Relaxed)
    }

    fn open(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        media_token: [u8; 32],
        initiate_connection: bool,
    ) -> Result<(), RadioApplicationError> {
        self.submit(MediaCommand::Open { contact_id, session_id, media_token, initiate_connection })
    }

    fn close(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
    ) -> Result<(), RadioApplicationError> {
        self.submit(MediaCommand::Close { contact_id, session_id })
    }

    fn request_floor(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        request_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError> {
        self.submit(MediaCommand::RequestFloor { contact_id, session_id, request_id })
    }

    fn end_burst(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        burst_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError> {
        self.submit(MediaCommand::EndBurst { contact_id, session_id, burst_id })
    }

    fn cancel_floor(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        request_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError> {
        self.submit(MediaCommand::CancelFloor { contact_id, session_id, request_id })
    }

    fn take_event(&mut self) -> Option<RadioSessionEvent> {
        self.events.try_recv().ok()
    }

    fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(waker);
        }
    }
}

enum MediaCommand {
    Open {
        contact_id: ContactId,
        session_id: RadioSessionId,
        media_token: [u8; 32],
        initiate_connection: bool,
    },
    Close {
        contact_id: ContactId,
        session_id: RadioSessionId,
    },
    RequestFloor {
        contact_id: ContactId,
        session_id: RadioSessionId,
        request_id: RadioOperationId,
    },
    EndBurst {
        contact_id: ContactId,
        session_id: RadioSessionId,
        burst_id: RadioOperationId,
    },
    CancelFloor {
        contact_id: ContactId,
        session_id: RadioSessionId,
        request_id: RadioOperationId,
    },
    Shutdown,
}

#[derive(Clone)]
struct PendingSession {
    contact_id: ContactId,
    session_id: RadioSessionId,
    media_token: [u8; 32],
    route: RadioMediaRoute,
    initiate_connection: bool,
    next_connect_at: Instant,
    reconnect_attempt: u8,
}

// These flags describe independent wire-level phases (authentication,
// floor ownership and playback drain); combining them into one enum would
// obscure valid combinations during reconnect and acknowledgement handling.
#[allow(clippy::struct_excessive_bools)]
struct LiveSession {
    pending: PendingSession,
    stream: Box<dyn RadioMediaStream>,
    cipher: Box<dyn RadioMediaCipher>,
    read_buffer: Vec<u8>,
    authenticated: bool,
    hello_sent: bool,
    local_is_coordinator: bool,
    pending_floor: Option<RadioOperationId>,
    pending_floor_at: Option<Instant>,
    local_burst: Option<RadioOperationId>,
    local_end_requested: Option<BurstEndReason>,
    local_floor_request: Option<RadioOperationId>,
    remote_burst: Option<RadioOperationId>,
    remote_floor_request: Option<RadioOperationId>,
    remote_floor_reservation: Option<(RadioOperationId, RadioOperationId)>,
    remote_playback_started: bool,
    remote_end_received: bool,
    remote_final_sequence_exclusive: Option<u32>,
    remote_received_sequences: BTreeSet<u32>,
    completed_bursts: VecDeque<RadioOperationId>,
    local_burst_started_at: Option<Instant>,
    remote_burst_started_at: Option<Instant>,
    transmit_sequence: u32,
    next_audio_send_at: Instant,
    jitter: JitterBuffer,
    last_received_at: Instant,
    last_media_activity_at: Instant,
    last_keep_alive_at: Instant,
    keep_alive_interval: Duration,
    keep_alive_sequence: u64,
    idle_keep_alives: u8,
    unacked_audio: VecDeque<(u32, Vec<u8>, Instant)>,
    oldest_unacked_at: Option<Instant>,
}

impl LiveSession {
    fn new(
        pending: PendingSession,
        stream: Box<dyn RadioMediaStream>,
        cipher: Box<dyn RadioMediaCipher>,
        keep_alive_interval: Duration,
    ) -> Result<Self, RadioApplicationError> {
        stream.configure(ACTIVE_READ_TIMEOUT, Duration::from_secs(2))?;
        let now = Instant::now();
        Ok(Self {
            local_is_coordinator: pending.route.local_identity < pending.route.remote_identity,
            pending,
            stream,
            cipher,
            read_buffer: Vec::with_capacity(MAX_RADIO_MEDIA_FRAME * 2),
            authenticated: false,
            hello_sent: false,
            pending_floor: None,
            pending_floor_at: None,
            local_burst: None,
            local_end_requested: None,
            local_floor_request: None,
            remote_burst: None,
            remote_floor_request: None,
            remote_floor_reservation: None,
            remote_playback_started: false,
            remote_end_received: false,
            remote_final_sequence_exclusive: None,
            remote_received_sequences: BTreeSet::new(),
            completed_bursts: VecDeque::new(),
            local_burst_started_at: None,
            remote_burst_started_at: None,
            transmit_sequence: 0,
            next_audio_send_at: now,
            jitter: JitterBuffer::default(),
            last_received_at: now,
            last_media_activity_at: now,
            last_keep_alive_at: now,
            keep_alive_interval,
            keep_alive_sequence: 0,
            idle_keep_alives: 0,
            unacked_audio: VecDeque::new(),
            oldest_unacked_at: None,
        })
    }

    fn matches(&self, contact_id: ContactId, session_id: RadioSessionId) -> bool {
        self.pending.contact_id == contact_id && self.pending.session_id == session_id
    }
}

struct MediaWorker {
    connector: Box<dyn RadioMediaConnector>,
    directory: Box<dyn RadioMediaDirectory>,
    audio: AudioPipeline,
    commands: Receiver<MediaCommand>,
    events: EventSink,
    pending: Option<PendingSession>,
    live: Option<LiveSession>,
    worker_alive: Arc<std::sync::atomic::AtomicBool>,
}

impl MediaWorker {
    fn new(
        connector: Box<dyn RadioMediaConnector>,
        directory: Box<dyn RadioMediaDirectory>,
        audio: AudioPipeline,
        commands: Receiver<MediaCommand>,
        events: SyncSender<RadioSessionEvent>,
        worker_alive: Arc<std::sync::atomic::AtomicBool>,
        waker: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
    ) -> Self {
        Self {
            connector,
            directory,
            audio,
            commands,
            events: EventSink { sender: events, waker: Arc::clone(&waker) },
            pending: None,
            live: None,
            worker_alive,
        }
    }

    fn run(mut self) {
        self.worker_alive.store(true, Ordering::Release);
        loop {
            match self.drain_commands() {
                Ok(true) => break,
                Ok(false) => {}
                Err(()) => {
                    eprintln!("torca-radio: media command failed; reconnecting session");
                    self.interrupt_live(RadioTransportFailure::Protocol);
                }
            }
            if self.live.is_none() {
                self.try_attach();
            }
            if self.live.is_some() && self.pump_live().is_err() {
                let contact = self
                    .live
                    .as_ref()
                    .map(|live| live.pending.contact_id.to_string())
                    .unwrap_or_else(|| "unknown".to_owned());
                eprintln!("torca-radio: media stream interrupted contact={contact}; reconnecting");
                let cause = self
                    .live
                    .as_ref()
                    .filter(|live| {
                        live.authenticated
                            && Instant::now().duration_since(live.last_received_at)
                                >= CONNECTION_IDLE_LIMIT
                    })
                    .map_or(RadioTransportFailure::StreamReset, |_| {
                        RadioTransportFailure::IdleTimeout
                    });
                self.interrupt_live(cause);
            }
            // The command channel is the idle wake source.  Active media and
            // pending connection deadlines retain the short audio cadence;
            // an unarmed radio waits for commands or an incoming listener
            // check instead of spinning/sleeping every 10 ms.
            let wait = self.next_wait_duration();
            let listener_only_wait = self.live.is_none()
                && self.pending.as_ref().is_none_or(|pending| !pending.initiate_connection);
            let command = if listener_only_wait {
                match self.commands.recv_timeout(Duration::from_secs(1)) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match self.commands.recv_timeout(wait) {
                    Ok(command) => Some(command),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            };
            if let Some(MediaCommand::Shutdown) = command {
                break;
            }
            if let Some(command) = command {
                if self.handle_command(command).is_err() {
                    eprintln!("torca-radio: media command failed; reconnecting session");
                    self.interrupt_live(RadioTransportFailure::Protocol);
                }
            }
        }
        self.shutdown_live(SessionCloseReason::Disabled);
        self.worker_alive.store(false, Ordering::Release);
    }

    fn next_wait_duration(&self) -> Duration {
        if let Some(live) = self.live.as_ref() {
            let now = Instant::now();
            let wait = active_media_wait(
                now,
                live.authenticated,
                live.local_burst.is_some()
                    || live.remote_burst.is_some()
                    || !live.unacked_audio.is_empty()
                    || !live.jitter.is_empty()
                    || !self.audio.outbound_is_empty()
                    || !self.audio.inbound_is_empty(),
                live.last_received_at,
                live.oldest_unacked_at,
                live.local_burst_started_at,
                live.last_media_activity_at,
                live.last_keep_alive_at,
                radio_keep_alive_interval(live.keep_alive_interval, live.idle_keep_alives),
            );
            // Align the next socket read timeout with the selected deadline;
            // otherwise a 20-ms stale timeout could postpone an ACK or burst
            // boundary that is due sooner.
            let _ = live.stream.set_read_deadline(wait.max(Duration::from_millis(1)));
            return wait;
        }
        if let Some(pending) = &self.pending {
            if pending.initiate_connection {
                return pending.next_connect_at.saturating_duration_since(Instant::now());
            }
            return pending.next_connect_at.saturating_duration_since(Instant::now());
        }
        // An unarmed listener or an accepted-session wait is woken by the
        // listener callback; `run` uses a blocking recv in this state.
        Duration::from_secs(1)
    }

    fn drain_commands(&mut self) -> Result<bool, ()> {
        loop {
            match self.commands.try_recv() {
                Ok(MediaCommand::Shutdown) | Err(TryRecvError::Disconnected) => return Ok(true),
                Err(TryRecvError::Empty) => return Ok(false),
                Ok(command) => self.handle_command(command)?,
            }
        }
    }

    fn handle_command(&mut self, command: MediaCommand) -> Result<(), ()> {
        match command {
            MediaCommand::Open { contact_id, session_id, media_token, initiate_connection } => {
                self.shutdown_live(SessionCloseReason::Replaced);
                let Some(route) = self.directory.route(contact_id) else {
                    emit(
                        &self.events,
                        RadioSessionEvent::Interrupted {
                            contact_id,
                            session_id: Some(session_id),
                            cause: RadioTransportFailure::EndpointUnavailable,
                            at: now_timestamp(),
                        },
                    );
                    self.pending = None;
                    return Ok(());
                };
                self.pending = Some(PendingSession {
                    contact_id,
                    session_id,
                    media_token,
                    route,
                    initiate_connection,
                    next_connect_at: Instant::now(),
                    reconnect_attempt: 0,
                });
            }
            MediaCommand::Close { contact_id, session_id } => {
                if self.live.as_ref().is_some_and(|live| live.matches(contact_id, session_id))
                    || self.pending.as_ref().is_some_and(|pending| {
                        pending.contact_id == contact_id && pending.session_id == session_id
                    })
                {
                    self.shutdown_live(SessionCloseReason::Disabled);
                    self.pending = None;
                }
            }
            MediaCommand::RequestFloor { contact_id, session_id, request_id } => {
                let Some(live) =
                    self.live.as_mut().filter(|live| live.matches(contact_id, session_id))
                else {
                    emit(&self.events, RadioSessionEvent::FloorDenied { contact_id, request_id });
                    return Ok(());
                };
                if !live.authenticated
                    || live.local_burst.is_some()
                    || live.remote_burst.is_some()
                    || live.remote_floor_reservation.is_some()
                {
                    emit(&self.events, RadioSessionEvent::FloorDenied { contact_id, request_id });
                    return Ok(());
                }
                live.pending_floor = Some(request_id);
                live.pending_floor_at = Some(Instant::now());
                if live.local_is_coordinator {
                    let burst_id = random_operation_id()?;
                    send_frame(
                        live,
                        &RadioMediaFrame::BurstStart {
                            request_id: request_id.to_opaque(),
                            burst_id: burst_id.to_opaque(),
                            max_duration_ms: MAX_RADIO_BURST_MS,
                        },
                    )?;
                    live.pending_floor = None;
                    live.pending_floor_at = None;
                    live.local_burst = Some(burst_id);
                    live.local_end_requested = None;
                    live.local_floor_request = Some(request_id);
                    live.local_burst_started_at = Some(Instant::now());
                    live.transmit_sequence = 0;
                    live.next_audio_send_at = Instant::now();
                    emit(
                        &self.events,
                        RadioSessionEvent::FloorGranted {
                            contact_id,
                            request_id,
                            burst_id,
                            at: now_timestamp(),
                        },
                    );
                } else {
                    send_frame(
                        live,
                        &RadioMediaFrame::FloorRequest { request_id: request_id.to_opaque() },
                    )?;
                }
            }
            MediaCommand::EndBurst { contact_id, session_id, burst_id } => {
                let Some(live) =
                    self.live.as_mut().filter(|live| live.matches(contact_id, session_id))
                else {
                    return Ok(());
                };
                if live.local_burst == Some(burst_id) {
                    // Capture is stopped by the coordinator before this command
                    // is submitted. Keep the burst open until every frame that
                    // the real-time callback already queued has reached the Tor
                    // stream; otherwise a quick PTT release truncates its tail.
                    live.local_end_requested = Some(BurstEndReason::Released);
                }
            }
            MediaCommand::CancelFloor { contact_id, session_id, request_id } => {
                let Some(live) =
                    self.live.as_mut().filter(|live| live.matches(contact_id, session_id))
                else {
                    return Ok(());
                };
                if live.pending_floor == Some(request_id) {
                    send_frame(
                        live,
                        &RadioMediaFrame::FloorDenied {
                            request_id: request_id.to_opaque(),
                            reason: FloorDeniedReason::Cancelled,
                        },
                    )?;
                    live.pending_floor = None;
                    live.pending_floor_at = None;
                } else if live.remote_floor_reservation.map(|(id, _)| id) == Some(request_id) {
                    live.remote_floor_reservation = None;
                    live.remote_floor_request = None;
                } else if live.local_floor_request == Some(request_id) {
                    if let Some(burst_id) = live.local_burst.take() {
                        remember_completed_burst(&mut live.completed_bursts, burst_id);
                        send_frame(
                            live,
                            &RadioMediaFrame::EndBurst {
                                burst_id: burst_id.to_opaque(),
                                final_sequence_exclusive: live.transmit_sequence,
                                reason: BurstEndReason::Released,
                            },
                        )?;
                    }
                    live.unacked_audio.clear();
                    live.oldest_unacked_at = None;
                    live.local_floor_request = None;
                    live.local_end_requested = None;
                    live.local_burst_started_at = None;
                    self.audio.clear();
                }
            }
            MediaCommand::Shutdown => return Err(()),
        }
        Ok(())
    }

    fn try_attach(&mut self) {
        let Some(pending) = self.pending.clone() else {
            return;
        };
        let started = Instant::now();
        let stream = if pending.initiate_connection {
            if Instant::now() < pending.next_connect_at {
                return;
            }
            eprintln!(
                "torca-radio: media connect started contact={} attempt={} virtual_port={}",
                pending.contact_id,
                pending.reconnect_attempt.saturating_add(1),
                TOR_RADIO_VIRTUAL_PORT,
            );
            match self.connector.connect(&pending.route, CONNECT_TIMEOUT) {
                Ok(stream) => {
                    eprintln!(
                        "torca-radio: media connect succeeded contact={} attempt={} elapsed_ms={}",
                        pending.contact_id,
                        pending.reconnect_attempt.saturating_add(1),
                        started.elapsed().as_millis(),
                    );
                    Some(stream)
                }
                Err(error) => {
                    eprintln!(
                        "torca-radio: media connect failed contact={} attempt={} elapsed_ms={} error={error:?}",
                        pending.contact_id,
                        pending.reconnect_attempt.saturating_add(1),
                        started.elapsed().as_millis(),
                    );
                    if let Some(current) = self.pending.as_mut() {
                        schedule_reconnect(current);
                    }
                    None
                }
            }
        } else {
            self.connector.try_accept().ok().flatten()
        };
        let Some(stream) = stream else {
            return;
        };
        let Ok(cipher) = self.directory.session_cipher(
            pending.contact_id,
            pending.session_id,
            &pending.media_token,
        ) else {
            return;
        };
        let keep_alive_interval = self.connector.keep_alive_interval();
        match LiveSession::new(pending, stream, cipher, keep_alive_interval) {
            Ok(live) => self.live = Some(live),
            Err(_) => self.interrupt_live(RadioTransportFailure::Protocol),
        }
    }

    fn pump_live(&mut self) -> Result<(), ()> {
        let live = self.live.as_mut().ok_or(())?;
        if !live.hello_sent {
            send_hello(live)?;
            live.hello_sent = true;
        }
        let frames = read_frames(live)?;
        for frame in frames {
            handle_incoming(live, &self.events, &self.audio, frame)?;
        }
        let now = Instant::now();
        if let (Some(request_id), Some(started_at)) = (live.pending_floor, live.pending_floor_at)
            && now.duration_since(started_at) >= FLOOR_REQUEST_TIMEOUT
        {
            // A lost grant must not leave the UI in `requesting` forever.
            // Clear the local reservation and publish a terminal transition;
            // the coordinator may retry explicitly with a new operation id.
            live.pending_floor = None;
            live.pending_floor_at = None;
            self.events.emit(RadioSessionEvent::FloorDenied {
                contact_id: live.pending.contact_id,
                request_id,
            });
        }
        if live.authenticated && now.duration_since(live.last_received_at) >= CONNECTION_IDLE_LIMIT
        {
            return Err(());
        }
        if live.authenticated
            && now.duration_since(live.last_media_activity_at)
                >= radio_keep_alive_interval(live.keep_alive_interval, live.idle_keep_alives)
            && now.duration_since(live.last_keep_alive_at)
                >= radio_keep_alive_interval(live.keep_alive_interval, live.idle_keep_alives)
        {
            live.keep_alive_sequence = live.keep_alive_sequence.saturating_add(1);
            send_frame(live, &RadioMediaFrame::KeepAlive { sequence: live.keep_alive_sequence })?;
            live.last_keep_alive_at = now;
            live.idle_keep_alives = live.idle_keep_alives.saturating_add(1);
        }
        if live.authenticated {
            enforce_burst_deadlines(live, &self.events, &self.audio)?;
            send_audio(live, &self.audio)?;
            resend_unacked_audio(live)?;
            finish_local_burst_if_drained(live, &self.events, &self.audio)?;
            fill_playback_queue(&mut live.jitter, &self.audio);
            finish_remote_burst_if_drained(live, &self.events, &self.audio);
        }
        Ok(())
    }

    fn interrupt_live(&mut self, cause: RadioTransportFailure) {
        let contact_id = self
            .live
            .as_ref()
            .map(|live| live.pending.contact_id)
            .or_else(|| self.pending.as_ref().map(|pending| pending.contact_id));
        let session_id = self
            .live
            .as_ref()
            .map(|live| live.pending.session_id)
            .or_else(|| self.pending.as_ref().map(|pending| pending.session_id));
        if let Some(live) = self.live.take() {
            let _ = live.stream.close_stream();
            let mut pending = live.pending;
            schedule_reconnect(&mut pending);
            self.pending = Some(pending);
        }
        self.audio.clear();
        if let Some(contact_id) = contact_id {
            self.events.emit(RadioSessionEvent::Interrupted {
                contact_id,
                session_id,
                cause,
                at: now_timestamp(),
            });
        }
    }

    fn shutdown_live(&mut self, reason: SessionCloseReason) {
        if let Some(mut live) = self.live.take() {
            let _ = send_frame(&mut live, &RadioMediaFrame::Close { reason });
            let _ = live.stream.close_stream();
        }
        self.audio.clear();
    }
}

#[derive(Clone)]
struct EventSink {
    sender: SyncSender<RadioSessionEvent>,
    waker: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
}

impl EventSink {
    fn emit(&self, event: RadioSessionEvent) {
        if self.sender.try_send(event).is_ok() {
            if let Some(callback) = self.waker.lock().ok().and_then(|slot| slot.clone()) {
                callback();
            }
        } else {
            eprintln!("torca-radio: media event queue full; transition dropped");
        }
    }
}

fn active_media_wait(
    now: Instant,
    authenticated: bool,
    audio_active: bool,
    last_received_at: Instant,
    oldest_unacked_at: Option<Instant>,
    local_burst_started_at: Option<Instant>,
    last_media_activity_at: Instant,
    last_keep_alive_at: Instant,
    keep_alive_interval: Duration,
) -> Duration {
    // A 20-ms deadline is required only while audio is actually moving.
    // Keeping it for an authenticated but idle session woke the worker about
    // 50 times per second and was the largest avoidable radio battery cost.
    let mut deadline = if audio_active {
        now + AUDIO_FRAME_INTERVAL
    } else {
        now + ACTIVE_READ_TIMEOUT.max(keep_alive_interval)
    };
    if authenticated {
        deadline = deadline.min(last_received_at + CONNECTION_IDLE_LIMIT);
        if let Some(oldest) = oldest_unacked_at {
            deadline = deadline.min(oldest + AUDIO_RETRANSMIT_AFTER);
        }
        if let Some(started) = local_burst_started_at {
            deadline = deadline.min(started + Duration::from_millis(MAX_RADIO_BURST_MS.into()));
        }
        let keep_alive_at = (last_media_activity_at + keep_alive_interval)
            .max(last_keep_alive_at + keep_alive_interval);
        deadline = deadline.min(keep_alive_at);
    }
    deadline.saturating_duration_since(now)
}

fn radio_keep_alive_interval(base: Duration, idle_keep_alives: u8) -> Duration {
    // `base` is a provider contract, not a hint.  In particular Iroh's QUIC
    // idle budget is shorter than the old adaptive 10s -> 20s -> 40s schedule;
    // the 40s gap deterministically caused Ready -> Reconnecting after about
    // 30s.  Keep the argument so the worker can retain its activity counters
    // and diagnostics without allowing them to invalidate the transport.
    let _ = idle_keep_alives;
    base.min(KEEP_ALIVE_MAX_INTERVAL)
}

fn fill_playback_queue(jitter: &mut JitterBuffer, audio: &AudioPipeline) {
    while audio.inbound_has_capacity() {
        let Some(frame) = jitter.pop() else {
            break;
        };
        if !audio.try_push_inbound(frame) {
            break;
        }
    }
}

fn finish_local_burst_if_drained(
    live: &mut LiveSession,
    events: &EventSink,
    audio: &AudioPipeline,
) -> Result<(), ()> {
    let Some(reason) = live.local_end_requested else {
        return Ok(());
    };
    if !audio.outbound_is_empty() {
        return Ok(());
    }
    if !live.unacked_audio.is_empty() {
        resend_unacked_audio(live)?;
        return Ok(());
    }
    let Some(burst_id) = live.local_burst else {
        live.local_end_requested = None;
        return Ok(());
    };
    send_frame(
        live,
        &RadioMediaFrame::EndBurst {
            burst_id: burst_id.to_opaque(),
            final_sequence_exclusive: live.transmit_sequence,
            reason,
        },
    )?;
    remember_completed_burst(&mut live.completed_bursts, burst_id);
    live.local_burst = None;
    live.local_end_requested = None;
    live.local_floor_request = None;
    live.local_burst_started_at = None;
    live.pending_floor = None;
    live.pending_floor_at = None;
    emit(events, RadioSessionEvent::BurstEnded { contact_id: live.pending.contact_id });
    Ok(())
}

fn finish_remote_burst_if_drained(
    live: &mut LiveSession,
    events: &EventSink,
    audio: &AudioPipeline,
) {
    if !live.remote_end_received || !live.jitter.is_empty() {
        return;
    }
    if let Some(final_sequence_exclusive) = live.remote_final_sequence_exclusive
        && (0..final_sequence_exclusive)
            .any(|sequence| !live.remote_received_sequences.contains(&sequence))
    {
        return;
    }
    if live.remote_playback_started {
        audio.request_end_cue();
        if !audio.playback_finished_after_end_cue() {
            return;
        }
    }
    if let Some(burst_id) = live.remote_burst {
        remember_completed_burst(&mut live.completed_bursts, burst_id);
    }
    live.remote_burst = None;
    live.remote_floor_request = None;
    live.remote_playback_started = false;
    live.remote_end_received = false;
    live.remote_final_sequence_exclusive = None;
    live.remote_received_sequences.clear();
    live.jitter.reset();
    emit(events, RadioSessionEvent::BurstEnded { contact_id: live.pending.contact_id });
}

fn send_hello(live: &mut LiveSession) -> Result<(), ()> {
    let mut nonce = [0_u8; 24];
    OsRng.try_fill_bytes(&mut nonce).map_err(|_| ())?;
    let proof =
        live.cipher.seal(nonce, &hello_aad(live.pending.session_id), &[]).map_err(|_| ())?;
    let proof: [u8; RADIO_MEDIA_PROOF_LEN] = proof.try_into().map_err(|_| ())?;
    send_frame(
        live,
        &RadioMediaFrame::Hello {
            protocol_version: RADIO_PROTOCOL_VERSION,
            session_id: live.pending.session_id.to_opaque(),
            nonce,
            proof,
        },
    )
}

fn handle_incoming(
    live: &mut LiveSession,
    events: &EventSink,
    audio: &AudioPipeline,
    frame: RadioMediaFrame,
) -> Result<(), ()> {
    live.last_received_at = Instant::now();
    if !live.authenticated {
        let RadioMediaFrame::Hello { protocol_version, session_id, nonce, proof } = frame else {
            return Err(());
        };
        if protocol_version != RADIO_PROTOCOL_VERSION {
            return Err(());
        }
        if session_id != live.pending.session_id.to_opaque() {
            return Err(());
        }
        live.cipher.open(nonce, &hello_aad(live.pending.session_id), &proof).map_err(|_| ())?;
        live.authenticated = true;
        live.pending.reconnect_attempt = 0;
        eprintln!(
            "torca-radio: media session ready contact={} session={}",
            live.pending.contact_id, live.pending.session_id,
        );
        emit(
            events,
            RadioSessionEvent::Ready {
                contact_id: live.pending.contact_id,
                session_id: live.pending.session_id,
                at: now_timestamp(),
            },
        );
        return Ok(());
    }
    if !matches!(&frame, RadioMediaFrame::KeepAlive { .. }) {
        live.last_media_activity_at = Instant::now();
        live.idle_keep_alives = 0;
    }
    match frame {
        RadioMediaFrame::Hello { .. } => return Err(()),
        RadioMediaFrame::FloorRequest { request_id } => {
            let request_id = RadioOperationId::from_opaque(request_id);
            // A lost response can cause the requester to repeat the same
            // floor request. Do not allocate a second burst for one request;
            // that would produce duplicate grants and duplicate start cues.
            if live.remote_floor_request == Some(request_id) {
                eprintln!(
                    "torca-radio: duplicate floor request contact={} request={:?}",
                    live.pending.contact_id, request_id
                );
                return Ok(());
            }
            if live.local_is_coordinator
                && live.local_burst.is_none()
                && live.remote_burst.is_none()
            {
                let burst_id = random_operation_id()?;
                send_frame(
                    live,
                    &RadioMediaFrame::FloorGrant {
                        request_id: request_id.to_opaque(),
                        burst_id: burst_id.to_opaque(),
                        max_duration_ms: MAX_RADIO_BURST_MS,
                    },
                )?;
                live.remote_floor_request = Some(request_id);
                live.remote_floor_reservation = Some((request_id, burst_id));
            } else {
                send_frame(
                    live,
                    &RadioMediaFrame::FloorDenied {
                        request_id: request_id.to_opaque(),
                        reason: FloorDeniedReason::ChannelBusy,
                    },
                )?;
            }
        }
        RadioMediaFrame::FloorGrant { request_id, burst_id, max_duration_ms } => {
            if max_duration_ms == 0 || max_duration_ms > MAX_RADIO_BURST_MS {
                return Err(());
            }
            let request_id = RadioOperationId::from_opaque(request_id);
            let burst_id = RadioOperationId::from_opaque(burst_id);
            if live.pending_floor == Some(request_id) {
                live.pending_floor = None;
                live.pending_floor_at = None;
                send_frame(
                    live,
                    &RadioMediaFrame::BurstStart {
                        request_id: request_id.to_opaque(),
                        burst_id: burst_id.to_opaque(),
                        max_duration_ms,
                    },
                )?;
                live.local_burst = Some(burst_id);
                live.local_end_requested = None;
                live.local_floor_request = Some(request_id);
                live.local_burst_started_at = Some(Instant::now());
                live.transmit_sequence = 0;
                live.next_audio_send_at = Instant::now();
                emit(
                    events,
                    RadioSessionEvent::FloorGranted {
                        contact_id: live.pending.contact_id,
                        request_id,
                        burst_id,
                        at: now_timestamp(),
                    },
                );
            }
        }
        RadioMediaFrame::BurstStart { request_id, burst_id, max_duration_ms } => {
            if max_duration_ms == 0 || max_duration_ms > MAX_RADIO_BURST_MS {
                return Err(());
            }
            let request_id = RadioOperationId::from_opaque(request_id);
            let burst_id = RadioOperationId::from_opaque(burst_id);
            // A control retransmission must not tear down an otherwise valid
            // media session or reset the receiver's jitter state. The sender
            // may repeat BurstStart when the first control response was lost.
            if live.remote_burst == Some(burst_id)
                || live.local_burst == Some(burst_id)
                || was_completed_burst(&live.completed_bursts, burst_id)
            {
                eprintln!(
                    "torca-radio: duplicate burst start contact={} burst={:?}",
                    live.pending.contact_id, burst_id
                );
                return Ok(());
            }
            let accepted = if live.local_is_coordinator {
                live.remote_floor_reservation == Some((request_id, burst_id))
            } else {
                live.local_burst.is_none() && live.remote_burst.is_none()
            };
            if !accepted {
                return Err(());
            }
            live.remote_floor_reservation = None;
            live.remote_floor_request = Some(request_id);
            live.remote_burst = Some(burst_id);
            live.remote_burst_started_at = Some(Instant::now());
            live.remote_playback_started = false;
            live.remote_end_received = false;
            live.remote_final_sequence_exclusive = None;
            live.remote_received_sequences.clear();
            live.jitter.reset();
            emit(
                events,
                RadioSessionEvent::RemoteBurstStarted {
                    contact_id: live.pending.contact_id,
                    burst_id,
                    at: now_timestamp(),
                },
            );
        }
        RadioMediaFrame::FloorDenied { request_id, reason: _ } => {
            let request_id = RadioOperationId::from_opaque(request_id);
            if live.pending_floor == Some(request_id) {
                live.pending_floor = None;
                live.pending_floor_at = None;
                emit(
                    events,
                    RadioSessionEvent::FloorDenied {
                        contact_id: live.pending.contact_id,
                        request_id,
                    },
                );
            } else if live.remote_floor_request == Some(request_id) {
                live.remote_floor_request = None;
                if let Some(burst_id) = live.remote_burst.take() {
                    live.remote_burst_started_at = None;
                    live.remote_playback_started = false;
                    live.jitter.reset();
                    audio.clear();
                    send_frame(
                        live,
                        &RadioMediaFrame::EndBurst {
                            burst_id: burst_id.to_opaque(),
                            final_sequence_exclusive: 0,
                            reason: BurstEndReason::Released,
                        },
                    )?;
                    emit(
                        events,
                        RadioSessionEvent::BurstEnded { contact_id: live.pending.contact_id },
                    );
                }
            }
        }
        RadioMediaFrame::Audio { burst_id, sequence, ciphertext } => {
            let burst_id = RadioOperationId::from_opaque(burst_id);
            if live.remote_burst != Some(burst_id) {
                if was_completed_burst(&live.completed_bursts, burst_id) {
                    eprintln!(
                        "torca-radio: late audio contact={} burst={:?} sequence={}",
                        live.pending.contact_id, burst_id, sequence
                    );
                    send_frame(
                        live,
                        &RadioMediaFrame::BurstAck { burst_id: burst_id.to_opaque(), sequence },
                    )?;
                    return Ok(());
                }
                return Err(());
            }
            // Audio retransmission is expected when an individual BurstAck is
            // lost. Keep acknowledging it, but never enqueue the same sequence
            // twice: doing so produces audible repeated syllables and makes
            // the jitter buffer wait for a sequence that was already played.
            if live.remote_received_sequences.contains(&sequence) {
                eprintln!(
                    "torca-radio: duplicate audio contact={} burst={:?} sequence={}",
                    live.pending.contact_id, burst_id, sequence
                );
                send_frame(
                    live,
                    &RadioMediaFrame::BurstAck { burst_id: burst_id.to_opaque(), sequence },
                )?;
                return Ok(());
            }
            let plaintext = live
                .cipher
                .open(
                    audio_nonce(burst_id, sequence),
                    &audio_aad(live.pending.session_id, burst_id, sequence),
                    &ciphertext,
                )
                .map_err(|_| ())?;
            let frame = plaintext.try_into().map_err(|_| ())?;
            if !live.remote_playback_started {
                live.remote_playback_started = true;
                emit(
                    events,
                    RadioSessionEvent::RemoteAudioStarted {
                        contact_id: live.pending.contact_id,
                        burst_id,
                    },
                );
            }
            live.jitter.push(sequence, frame);
            live.remote_received_sequences.insert(sequence);
            send_frame(
                live,
                &RadioMediaFrame::BurstAck { burst_id: burst_id.to_opaque(), sequence },
            )?;
        }
        RadioMediaFrame::EndBurst { burst_id, final_sequence_exclusive, reason: _ } => {
            let burst_id = RadioOperationId::from_opaque(burst_id);
            if live.remote_burst == Some(burst_id) {
                live.remote_burst_started_at = None;
                live.remote_end_received = true;
                live.remote_final_sequence_exclusive = Some(final_sequence_exclusive);
                live.jitter.finish();
            } else if live.local_burst == Some(burst_id) {
                remember_completed_burst(&mut live.completed_bursts, burst_id);
                live.local_burst = None;
                live.local_end_requested = None;
                live.local_floor_request = None;
                live.local_burst_started_at = None;
                live.pending_floor = None;
                live.pending_floor_at = None;
                live.unacked_audio.clear();
                live.oldest_unacked_at = None;
                live.remote_final_sequence_exclusive = None;
                live.remote_received_sequences.clear();
                audio.clear();
                emit(events, RadioSessionEvent::BurstEnded { contact_id: live.pending.contact_id });
            }
        }
        RadioMediaFrame::BurstAck { burst_id, sequence } => {
            let burst_id = RadioOperationId::from_opaque(burst_id);
            if live.local_burst == Some(burst_id) {
                // ACKs identify one authenticated frame, not a cumulative
                // prefix. This preserves an earlier frame that was lost while
                // a later frame arrived out of order.
                live.unacked_audio.retain(|(sent_sequence, _, _)| *sent_sequence != sequence);
                if live.unacked_audio.is_empty() {
                    live.oldest_unacked_at = None;
                }
            }
        }
        RadioMediaFrame::KeepAlive { .. } => {}
        RadioMediaFrame::Close { .. } => return Err(()),
    }
    Ok(())
}

fn send_audio(live: &mut LiveSession, audio: &AudioPipeline) -> Result<(), ()> {
    let Some(burst_id) = live.local_burst else {
        return Ok(());
    };
    let now = Instant::now();
    if now < live.next_audio_send_at {
        return Ok(());
    }
    let Some(frame) = audio.take_outbound() else {
        return Ok(());
    };
    let sequence = live.transmit_sequence;
    let ciphertext = live
        .cipher
        .seal(
            audio_nonce(burst_id, sequence),
            &audio_aad(live.pending.session_id, burst_id, sequence),
            &frame,
        )
        .map_err(|_| ())?;
    send_frame(
        live,
        &RadioMediaFrame::Audio {
            burst_id: burst_id.to_opaque(),
            sequence,
            ciphertext: ciphertext.clone(),
        },
    )?;
    // A ten-second burst is bounded to MAX_RADIO_BURST_FRAMES. Keep every
    // ciphertext until its exact ACK arrives; dropping the oldest 64 frames
    // could leave the receiver waiting forever for a missing early sequence.
    let sent_at = Instant::now();
    if live.unacked_audio.is_empty() {
        live.oldest_unacked_at = Some(sent_at);
    }
    live.unacked_audio.push_back((sequence, ciphertext, sent_at));
    while live.unacked_audio.len() > MAX_RADIO_BURST_FRAMES {
        live.unacked_audio.pop_front();
    }
    live.transmit_sequence = live.transmit_sequence.saturating_add(1);
    // A frame contains 20 ms of 8 kHz audio. Keep wire pacing tied to media
    // time instead of the worker's 10 ms maintenance tick; otherwise a
    // backlog is drained at 2x realtime, creating bursts of retransmits and
    // audible repetition on the receiving jitter buffer.
    live.next_audio_send_at = now + AUDIO_FRAME_INTERVAL;
    Ok(())
}

fn resend_unacked_audio(live: &mut LiveSession) -> Result<(), ()> {
    let Some(burst_id) = live.local_burst else {
        return Ok(());
    };
    if live.oldest_unacked_at.is_some_and(|sent_at| sent_at.elapsed() >= MAX_UNACKED_AUDIO_AGE) {
        return Err(());
    }
    let now = Instant::now();
    let mut retry = Vec::with_capacity(MAX_RETRANSMITS_PER_TICK);
    for (sequence, ciphertext, sent_at) in &mut live.unacked_audio {
        if retry.len() == MAX_RETRANSMITS_PER_TICK {
            break;
        }
        if now.duration_since(*sent_at) < AUDIO_RETRANSMIT_AFTER {
            continue;
        }
        retry.push((*sequence, ciphertext.clone()));
        *sent_at = now;
    }
    for (sequence, ciphertext) in retry {
        send_frame(
            live,
            &RadioMediaFrame::Audio { burst_id: burst_id.to_opaque(), sequence, ciphertext },
        )?;
    }
    Ok(())
}

fn enforce_burst_deadlines(
    live: &mut LiveSession,
    events: &EventSink,
    audio: &AudioPipeline,
) -> Result<(), ()> {
    let limit = Duration::from_millis(u64::from(MAX_RADIO_BURST_MS));
    if let (Some(_burst_id), Some(started_at)) = (live.local_burst, live.local_burst_started_at) {
        if started_at.elapsed() >= limit && live.local_end_requested.is_none() {
            // Stop accepting microphone frames, then use the normal drain and
            // ACK path. Sending EndBurst immediately would leave queued audio
            // unacknowledged and make the receiver wait for a sequence gap.
            live.local_end_requested = Some(BurstEndReason::LimitReached);
            audio.set_capture_enabled(false);
            audio.clear();
        }
    }
    if let (Some(burst_id), Some(started_at)) = (live.remote_burst, live.remote_burst_started_at) {
        if started_at.elapsed() >= limit {
            send_frame(
                live,
                &RadioMediaFrame::EndBurst {
                    burst_id: burst_id.to_opaque(),
                    final_sequence_exclusive: 0,
                    reason: BurstEndReason::LimitReached,
                },
            )?;
            remember_completed_burst(&mut live.completed_bursts, burst_id);
            live.remote_burst = None;
            live.remote_floor_request = None;
            live.remote_burst_started_at = None;
            live.remote_playback_started = false;
            live.remote_final_sequence_exclusive = None;
            live.remote_received_sequences.clear();
            live.jitter.reset();
            audio.clear();
            emit(events, RadioSessionEvent::BurstEnded { contact_id: live.pending.contact_id });
        }
    }
    Ok(())
}

fn send_frame(live: &mut LiveSession, frame: &RadioMediaFrame) -> Result<(), ()> {
    if !matches!(frame, RadioMediaFrame::KeepAlive { .. }) {
        live.last_media_activity_at = Instant::now();
        live.idle_keep_alives = 0;
    }
    let bytes = RadioMediaCodec::encode_framed(frame).map_err(|_| ())?;
    live.stream.write_all(&bytes).map_err(|_| ())?;
    // TcpStream writes are already handed to the kernel. Flushing every
    // 20-ms audio frame adds a syscall without improving delivery (TCP_NODELAY
    // is enabled on the stream). Control frames still flush immediately so
    // floor/burst transitions do not wait behind a buffered audio batch.
    if !matches!(frame, RadioMediaFrame::Audio { .. }) {
        live.stream.flush().map_err(|_| ())?;
    }
    Ok(())
}

fn read_frames(live: &mut LiveSession) -> Result<Vec<RadioMediaFrame>, ()> {
    let mut chunk = [0_u8; 1_024];
    match live.stream.read(&mut chunk) {
        Ok(0) => return Err(()),
        Ok(count) => {
            if live.read_buffer.len().saturating_add(count) > READ_BUFFER_LIMIT {
                return Err(());
            }
            live.read_buffer.extend_from_slice(&chunk[..count]);
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
            ) => {}
        Err(_) => return Err(()),
    }
    let mut output = Vec::with_capacity(2);
    let mut consumed = 0;
    loop {
        if live.read_buffer.len().saturating_sub(consumed) < 4 {
            break;
        }
        let length = u32::from_be_bytes(
            live.read_buffer[consumed..consumed + 4].try_into().expect("four-byte media prefix"),
        ) as usize;
        if length == 0 || length > MAX_RADIO_MEDIA_FRAME {
            return Err(());
        }
        if live.read_buffer.len().saturating_sub(consumed) < 4 + length {
            break;
        }
        let start = consumed + 4;
        let payload = live.read_buffer[start..start + length].to_vec();
        consumed = start + length;
        output.push(RadioMediaCodec::decode(&payload).map_err(|_| ())?);
    }
    if consumed > 0 {
        live.read_buffer.drain(..consumed);
    }
    Ok(output)
}

fn hello_aad(session_id: RadioSessionId) -> Vec<u8> {
    let mut aad = Vec::with_capacity(20);
    aad.extend_from_slice(b"radio-hello-v1");
    aad.extend_from_slice(session_id.to_opaque().as_bytes());
    aad
}

fn audio_aad(session_id: RadioSessionId, burst_id: RadioOperationId, sequence: u32) -> Vec<u8> {
    let mut aad = Vec::with_capacity(52);
    aad.extend_from_slice(b"radio-audio-v1");
    aad.extend_from_slice(session_id.to_opaque().as_bytes());
    aad.extend_from_slice(burst_id.to_opaque().as_bytes());
    aad.extend_from_slice(&sequence.to_be_bytes());
    aad
}

fn audio_nonce(burst_id: RadioOperationId, sequence: u32) -> [u8; 24] {
    let mut nonce = [0_u8; 24];
    nonce[..16].copy_from_slice(burst_id.to_opaque().as_bytes());
    nonce[16..20].copy_from_slice(&sequence.to_be_bytes());
    nonce[23] = 1;
    nonce
}

fn random_operation_id() -> Result<RadioOperationId, ()> {
    let mut bytes = [0_u8; 16];
    OsRng.try_fill_bytes(&mut bytes).map_err(|_| ())?;
    Ok(RadioOperationId::from_opaque(OpaqueId::from_bytes(bytes)))
}

fn now_timestamp() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(Timestamp::MIN_UNIX_MILLIS);
    Timestamp::from_unix_millis(millis).unwrap_or(Timestamp::UNIX_EPOCH)
}

/// Exponential full-jitter backoff keeps two peers from repeatedly redialing
/// in lockstep after one transient onion-stream failure.
fn schedule_reconnect(pending: &mut PendingSession) {
    let exponent = u32::from(pending.reconnect_attempt.min(4));
    let ceiling = RECONNECT_BASE_DELAY
        .checked_mul(1_u32 << exponent)
        .unwrap_or(RECONNECT_MAX_DELAY)
        .min(RECONNECT_MAX_DELAY);
    let ceiling_ms = u64::try_from(ceiling.as_millis()).unwrap_or(u64::MAX).max(1);
    let mut entropy = [0_u8; 2];
    let random = OsRng
        .try_fill_bytes(&mut entropy)
        .map(|()| u64::from(u16::from_be_bytes(entropy)))
        .unwrap_or(0);
    // Avoid a zero-delay busy loop while retaining full jitter within the
    // selected exponential ceiling.
    let delay_ms = 100 + random % ceiling_ms.max(100);
    pending.next_connect_at = Instant::now() + Duration::from_millis(delay_ms);
    pending.reconnect_attempt = pending.reconnect_attempt.saturating_add(1);
}

fn emit(sender: &EventSink, event: RadioSessionEvent) {
    sender.emit(event);
}

fn remember_completed_burst(history: &mut VecDeque<RadioOperationId>, burst_id: RadioOperationId) {
    if history.contains(&burst_id) {
        return;
    }
    history.push_back(burst_id);
    while history.len() > COMPLETED_BURST_HISTORY {
        let _ = history.pop_front();
    }
}

fn was_completed_burst(history: &VecDeque<RadioOperationId>, burst_id: RadioOperationId) -> bool {
    history.contains(&burst_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_media_sleeps_until_the_transport_deadline() {
        let now = Instant::now();
        assert_eq!(
            active_media_wait(now, false, false, now, None, None, now, now, KEEP_ALIVE_INTERVAL,),
            KEEP_ALIVE_INTERVAL
        );
    }

    #[test]
    fn active_media_selects_the_earliest_retransmit_or_burst_deadline() {
        let now = Instant::now();
        assert_eq!(
            active_media_wait(
                now,
                true,
                true,
                now + Duration::from_secs(10),
                Some(now.checked_sub(Duration::from_millis(247)).unwrap_or(now)),
                Some(
                    now.checked_sub(Duration::from_millis(u64::from(MAX_RADIO_BURST_MS) - 7))
                        .unwrap_or(now),
                ),
                now,
                now,
                KEEP_ALIVE_INTERVAL,
            ),
            Duration::from_millis(3)
        );
    }

    #[test]
    fn audio_nonce_is_stable_and_sequence_specific() {
        let burst = RadioOperationId::from_opaque(OpaqueId::from_u128(9));
        assert_eq!(audio_nonce(burst, 7), audio_nonce(burst, 7));
        assert_ne!(audio_nonce(burst, 7), audio_nonce(burst, 8));
    }

    #[test]
    fn audio_aad_binds_session_burst_and_sequence() {
        let first = audio_aad(
            RadioSessionId::from_opaque(OpaqueId::from_u128(1)),
            RadioOperationId::from_opaque(OpaqueId::from_u128(2)),
            3,
        );
        let second = audio_aad(
            RadioSessionId::from_opaque(OpaqueId::from_u128(1)),
            RadioOperationId::from_opaque(OpaqueId::from_u128(2)),
            4,
        );
        assert_ne!(first, second);
    }

    #[test]
    fn completed_burst_history_is_bounded_and_keeps_recent_ids() {
        let mut history = VecDeque::new();
        let ids: Vec<_> = (0..(COMPLETED_BURST_HISTORY + 2))
            .map(|value| RadioOperationId::from_opaque(OpaqueId::from_u128(value as u128)))
            .collect();
        for id in &ids {
            remember_completed_burst(&mut history, *id);
        }
        assert_eq!(history.len(), COMPLETED_BURST_HISTORY);
        assert!(!was_completed_burst(&history, ids[0]));
        assert!(was_completed_burst(&history, *ids.last().expect("history has ids")));
    }

    #[test]
    fn keep_alive_never_exceeds_provider_contract_after_idle_backoff() {
        let provider_interval = Duration::from_secs(10);
        assert_eq!(radio_keep_alive_interval(provider_interval, 0), provider_interval);
        assert_eq!(radio_keep_alive_interval(provider_interval, 1), provider_interval);
        assert_eq!(radio_keep_alive_interval(provider_interval, u8::MAX), provider_interval);
    }
}
