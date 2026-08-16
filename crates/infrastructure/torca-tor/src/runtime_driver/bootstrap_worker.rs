use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};

use super::recovery_epoch::RecoveryEpoch;
use super::timing::RESTART_BOOTSTRAP_TIMEOUT;
use super::{TorWake, notify_tor_wake};
use crate::{TorService, TorError};

/// One asynchronous Tor recovery attempt. Initial warm-up remains sequential;
/// recovery work is isolated so runtime maintenance never blocks on Arti bootstrap.
pub(super) struct TorBootstrapWorker {
    epoch: RecoveryEpoch,
    receiver: Receiver<(RecoveryEpoch, Result<TorService, TorError>)>,
    worker: Option<JoinHandle<()>>,
}

impl TorBootstrapWorker {
    pub(super) fn spawn(
        epoch: RecoveryEpoch,
        state_root: PathBuf,
        previous_client: Option<Arc<TorService>>,
        wake: TorWake,
    ) -> Result<Self, std::io::Error> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new().name("torca-tor-recovery".into()).spawn(move || {
            // The old runtime owns locks below the same Arti state root. Drop it
            // in this background lane before constructing the replacement so
            // maintenance and the ABI actor never block on runtime shutdown.
            drop(previous_client);
            let result = TorService::bootstrap(state_root, RESTART_BOOTSTRAP_TIMEOUT);
            let _ = sender.send((epoch, result));
            notify_tor_wake(&wake);
        })?;
        Ok(Self { epoch, receiver, worker: Some(worker) })
    }

    pub(super) fn try_take_result(
        &mut self,
    ) -> Option<(RecoveryEpoch, Result<TorService, TorError>)> {
        match self.receiver.try_recv() {
            Ok(result) => {
                if let Some(worker) = self.worker.take() {
                    let _ = worker.join();
                }
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some((
                self.epoch,
                Err(TorError("Tor recovery worker disconnected".into())),
            )),
        }
    }
}
