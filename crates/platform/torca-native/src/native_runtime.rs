use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use torca_bridge::{BridgeCommand, EngineBridge};
use torca_client_engine::{ClientEngineActor, EngineHandle};
use torca_runtime_host::{HostTorState, RuntimeHostHandle, RuntimeHostOwner};

use crate::composition::{NativeCompositionError, spawn_production_engine};
use crate::json::{bridge_result_json, bridge_snapshot_json, empty_snapshot_json, error_result, success_result};
use crate::notification_json::notification_snapshot_json;
use crate::runtime_composition::spawn_production_host;

pub(crate) const ABI_OK: i32 = 0;
pub(crate) const ABI_ERROR: i32 = -1;
pub(crate) const ABI_CLOSED: i32 = -2;
const NETWORK_RETRY_DELAY: Duration = Duration::from_secs(5);

type HostStartResult = Result<(RuntimeHostHandle, RuntimeHostOwner), NativeCompositionError>;

pub struct NativeEngineRuntime {
    engine: EngineHandle,
    bridge: EngineBridge,
    actor: Option<ClientEngineActor>,
    host: Option<RuntimeHostOwner>,
    host_start: Option<Receiver<HostStartResult>>,
    host_retry_at: Option<Instant>,
    host_state_hint: HostTorState,
    pub(crate) last_result_json: String,
    pub(crate) snapshot_json: String,
    pub(crate) diagnostics_json: String,
}

impl NativeEngineRuntime {
    pub(crate) fn new() -> Result<Self, ()> {
        let (engine, actor) = spawn_production_engine().map_err(|_| ())?;
        let bridge = EngineBridge::new(engine.clone());
        let mut runtime = Self {
            engine,
            bridge,
            actor: Some(actor),
            host: None,
            host_start: None,
            host_retry_at: None,
            host_state_hint: HostTorState::Stopped,
            last_result_json: success_result("initialized"),
            snapshot_json: empty_snapshot_json(),
            diagnostics_json: "{\"events\":[]}".into(),
        };
        if runtime.has_identity()? {
            runtime.begin_runtime_start();
        }
        if runtime.refresh_snapshot() != ABI_OK { return Err(()); }
        Ok(runtime)
    }

    pub(crate) fn execute(&mut self, command: BridgeCommand) -> i32 {
        if self.is_closed() {
            self.last_result_json = error_result("native engine is closed");
            return ABI_CLOSED;
        }
        self.advance_runtime_start();
        let creates_identity = matches!(&command, BridgeCommand::CreateIdentity { .. });
        let result = self.bridge.execute(command);
        if !result.ok {
            self.last_result_json = bridge_result_json(&result);
            return ABI_ERROR;
        }
        if creates_identity && !self.bridge.has_runtime() {
            self.begin_runtime_start();
        }
        self.last_result_json = bridge_result_json(&result);
        let _ = self.refresh_snapshot();
        ABI_OK
    }

    pub(crate) fn refresh_snapshot(&mut self) -> i32 {
        if self.is_closed() { return ABI_CLOSED; }
        self.advance_runtime_start();
        match self.bridge.snapshot() {
            Ok(mut snapshot) => {
                if !self.bridge.has_runtime() && snapshot.identity_name.is_some() {
                    snapshot.tor_state = format!("{:?}", self.host_state_hint).to_lowercase();
                }
                self.snapshot_json = bridge_snapshot_json(&snapshot);
                ABI_OK
            }
            Err(error) => {
                // Snapshot queries are intentionally bounded. Keep the previous readable snapshot
                // if the network host is busy instead of blocking Flutter on a Tor/peer operation.
                self.last_result_json = error_result(&error.to_string());
                ABI_ERROR
            }
        }
    }

    pub(crate) fn notification_snapshot_json(&self) -> Result<String, ()> {
        if self.is_closed() { return Err(()); }
        self.bridge.snapshot().map(|snapshot| notification_snapshot_json(&snapshot)).map_err(|_| ())
    }

    pub(crate) fn refresh_diagnostics(&mut self) -> i32 {
        if self.is_closed() { return ABI_CLOSED; }
        self.advance_runtime_start();
        match self.bridge.diagnostics_json() {
            Ok(value) => { self.diagnostics_json = value; ABI_OK }
            Err(error) => { self.last_result_json = error_result(&error.to_string()); ABI_ERROR }
        }
    }

    pub(crate) fn reject_argument(&mut self, error: &'static str) -> i32 {
        self.last_result_json = error_result(error);
        ABI_ERROR
    }

    pub(crate) fn close(&mut self) -> i32 {
        self.host_retry_at = None;
        self.host_state_hint = HostTorState::Stopped;
        // Dropping the receiver tells an in-flight startup thread to shut down a host it may
        // finish constructing after the process runtime has already been closed.
        self.host_start = None;
        if let Some(host) = self.host.take() {
            if host.shutdown().is_err() {
                self.last_result_json = error_result("secure runtime shutdown failed");
            }
        }
        let Some(actor) = self.actor.take() else { return ABI_OK; };
        match actor.shutdown() {
            Ok(()) => ABI_OK,
            Err(error) => { self.last_result_json = error_result(&error.to_string()); ABI_ERROR }
        }
    }

    fn begin_runtime_start(&mut self) {
        if self.is_closed() || self.host.is_some() || self.host_start.is_some() {
            return;
        }
        self.host_retry_at = None;
        self.host_state_hint = HostTorState::Starting;
        let engine = self.engine.clone();
        let (sender, receiver) = mpsc::channel::<HostStartResult>();
        self.host_start = Some(receiver);
        thread::spawn(move || {
            let result = spawn_production_host(engine);
            if let Err(send_error) = sender.send(result) {
                if let Ok((_handle, owner)) = send_error.0 {
                    let _ = owner.shutdown();
                }
            }
        });
    }

    fn advance_runtime_start(&mut self) {
        let outcome = match self.host_start.as_ref() {
            Some(receiver) => match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err(NativeCompositionError::new(
                    "network runtime startup worker disconnected",
                ))),
            },
            None => None,
        };

        if let Some(result) = outcome {
            self.host_start = None;
            match result {
                Ok((handle, owner)) => {
                    self.bridge.attach_runtime(handle);
                    self.host = Some(owner);
                    self.host_retry_at = None;
                    self.host_state_hint = HostTorState::Ready;
                }
                Err(_) => {
                    self.host_state_hint = HostTorState::Degraded;
                    self.host_retry_at = Some(Instant::now() + NETWORK_RETRY_DELAY);
                }
            }
        }

        if self.host.is_none()
            && self.host_start.is_none()
            && self.host_retry_at.is_some_and(|deadline| Instant::now() >= deadline)
            && self.has_identity().unwrap_or(false)
        {
            self.begin_runtime_start();
        }
    }

    fn has_identity(&self) -> Result<bool, ()> {
        self.engine.snapshot().map(|snapshot| snapshot.identity.is_some()).map_err(|_| ())
    }

    fn is_closed(&self) -> bool { self.actor.is_none() }
}
impl Drop for NativeEngineRuntime {
    fn drop(&mut self) { let _ = self.close(); }
}
