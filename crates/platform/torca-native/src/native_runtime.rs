use torca_bridge::{BridgeCommand, EngineBridge};
use torca_client_engine::{ClientEngineActor, EngineHandle};
use torca_runtime_host::RuntimeHostOwner;

use crate::composition::spawn_production_engine;
use crate::json::{bridge_result_json, bridge_snapshot_json, empty_snapshot_json, error_result, success_result};
use crate::runtime_composition::spawn_production_host;

pub(crate) const ABI_OK: i32 = 0;
pub(crate) const ABI_ERROR: i32 = -1;
pub(crate) const ABI_CLOSED: i32 = -2;

pub struct NativeEngineRuntime {
    engine: EngineHandle,
    bridge: EngineBridge,
    actor: Option<ClientEngineActor>,
    host: Option<RuntimeHostOwner>,
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
            last_result_json: success_result("initialized"),
            snapshot_json: empty_snapshot_json(),
            diagnostics_json: "{\"events\":[]}".into(),
        };
        if runtime.engine.snapshot().map_err(|_| ())?.identity.is_some() {
            runtime.ensure_runtime().map_err(|_| ())?;
        }
        if runtime.refresh_snapshot() != ABI_OK { return Err(()); }
        Ok(runtime)
    }

    pub(crate) fn execute(&mut self, command: BridgeCommand) -> i32 {
        if self.is_closed() {
            self.last_result_json = error_result("native engine is closed");
            return ABI_CLOSED;
        }
        let creates_identity = matches!(&command, BridgeCommand::CreateIdentity { .. });
        let result = self.bridge.execute(command);
        if !result.ok {
            self.last_result_json = bridge_result_json(&result);
            return ABI_ERROR;
        }
        if creates_identity && !self.bridge.has_runtime() {
            if let Err(error) = self.ensure_runtime() {
                self.last_result_json = error_result(error);
                let _ = self.refresh_snapshot();
                return ABI_ERROR;
            }
        }
        self.last_result_json = bridge_result_json(&result);
        let _ = self.refresh_snapshot();
        ABI_OK
    }

    pub(crate) fn refresh_snapshot(&mut self) -> i32 {
        if self.is_closed() { return ABI_CLOSED; }
        match self.bridge.snapshot() {
            Ok(snapshot) => { self.snapshot_json = bridge_snapshot_json(&snapshot); ABI_OK }
            Err(error) => { self.last_result_json = error_result(&error.to_string()); ABI_ERROR }
        }
    }

    pub(crate) fn refresh_diagnostics(&mut self) -> i32 {
        if self.is_closed() { return ABI_CLOSED; }
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

    fn ensure_runtime(&mut self) -> Result<(), &'static str> {
        if self.host.is_some() { return Ok(()); }
        let (handle, owner) = spawn_production_host(self.engine.clone())
            .map_err(|_| "secure network runtime initialization failed")?;
        self.bridge.attach_runtime(handle);
        self.host = Some(owner);
        Ok(())
    }

    fn is_closed(&self) -> bool { self.actor.is_none() }
}
impl Drop for NativeEngineRuntime {
    fn drop(&mut self) { let _ = self.close(); }
}
