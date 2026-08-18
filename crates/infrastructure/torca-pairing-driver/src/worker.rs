use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use torca_foundation::Timestamp;
use torca_pairing::{PairingCode, PairingSessionId};
use torca_runtime::{PairingDriver, PairingInvitationView, RuntimeDriverError};

const INTERACTIVE_REPLY_WAIT: Duration = Duration::from_secs(8);

enum PairingWorkerCommand {
    Create {
        session_id: PairingSessionId,
        now: Timestamp,
        reply: SyncSender<Result<PairingInvitationView, RuntimeDriverError>>,
    },
    Join {
        session_id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
        now: Timestamp,
        reply: SyncSender<Result<(), RuntimeDriverError>>,
    },
    Approve {
        session_id: PairingSessionId,
        now: Timestamp,
        reply: SyncSender<Result<(), RuntimeDriverError>>,
    },
    Reject {
        session_id: PairingSessionId,
        reply: SyncSender<Result<(), RuntimeDriverError>>,
    },
    Cancel {
        session_id: PairingSessionId,
        reply: SyncSender<Result<(), RuntimeDriverError>>,
    },
    NetworkChanged(Timestamp),
    Shutdown,
}

/// Isolates relay I/O from the main runtime actor. Periodic polls are coalesced in a bounded
/// mailbox, so a slow Tor circuit cannot freeze snapshots, message delivery, or the UI.
pub struct PairingWorkerDriver {
    sender: SyncSender<PairingWorkerCommand>,
    worker: Option<JoinHandle<()>>,
}

impl PairingWorkerDriver {
    pub fn spawn<D: PairingDriver>(mut driver: D) -> Result<Self, RuntimeDriverError> {
        let (sender, receiver) = mpsc::sync_channel(8);
        let worker = std::thread::Builder::new()
            .name("torca-pairing-supervisor".to_owned())
            .spawn(move || run_pairing_worker(&mut driver, &receiver))
            .map_err(|_| RuntimeDriverError::Pairing)?;
        Ok(Self { sender, worker: Some(worker) })
    }

    fn request<T>(
        &self,
        build: impl FnOnce(SyncSender<Result<T, RuntimeDriverError>>) -> PairingWorkerCommand,
    ) -> Result<T, RuntimeDriverError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        match self.sender.try_send(build(reply)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => return Err(RuntimeDriverError::Pending),
            Err(TrySendError::Disconnected(_)) => return Err(RuntimeDriverError::Pairing),
        }
        receiver.recv_timeout(INTERACTIVE_REPLY_WAIT).map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => RuntimeDriverError::Pending,
            mpsc::RecvTimeoutError::Disconnected => RuntimeDriverError::Pairing,
        })?
    }
}

impl PairingDriver for PairingWorkerDriver {
    fn create(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<PairingInvitationView, RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Create { session_id, now, reply })
    }

    fn join(
        &mut self,
        session_id: PairingSessionId,
        code: PairingCode,
        ticket: Option<[u8; 16]>,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Join { session_id, code, ticket, now, reply })
    }

    fn approve(
        &mut self,
        session_id: PairingSessionId,
        now: Timestamp,
    ) -> Result<(), RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Approve { session_id, now, reply })
    }

    fn reject(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Reject { session_id, reply })
    }

    fn cancel(&mut self, session_id: PairingSessionId) -> Result<(), RuntimeDriverError> {
        self.request(|reply| PairingWorkerCommand::Cancel { session_id, reply })
    }

    fn maintenance(&mut self, now: Timestamp) -> Result<(), RuntimeDriverError> {
        let _ = now;
        // The worker owns its own deadline-driven poll/maintenance clock.
        // RuntimeOwner still calls this trait method for compatibility, but it
        // must not enqueue a periodic message behind interactive relay I/O.
        Ok(())
    }

    fn network_changed(&mut self, now: Timestamp) {
        let _ = self.sender.try_send(PairingWorkerCommand::NetworkChanged(now));
    }

    fn shutdown(&mut self) {
        let _ = self.sender.send(PairingWorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_pairing_worker<D: PairingDriver>(driver: &mut D, receiver: &Receiver<PairingWorkerCommand>) {
    loop {
        let timeout = worker_timestamp()
            .and_then(|now| driver.next_maintenance_delay(now))
            .unwrap_or(Duration::from_secs(3_600));
        let command = match receiver.recv_timeout(timeout) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(now) = worker_timestamp() {
                    let _ = driver.maintenance(now);
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match command {
            PairingWorkerCommand::Create { session_id, now, reply } => {
                let _ = reply.send(driver.create(session_id, now));
            }
            PairingWorkerCommand::Join { session_id, code, ticket, now, reply } => {
                let _ = reply.send(driver.join(session_id, code, ticket, now));
            }
            PairingWorkerCommand::Approve { session_id, now, reply } => {
                let _ = reply.send(driver.approve(session_id, now));
            }
            PairingWorkerCommand::Reject { session_id, reply } => {
                let _ = reply.send(driver.reject(session_id));
            }
            PairingWorkerCommand::Cancel { session_id, reply } => {
                let _ = reply.send(driver.cancel(session_id));
            }
            PairingWorkerCommand::NetworkChanged(now) => driver.network_changed(now),
            PairingWorkerCommand::Shutdown => {
                driver.shutdown();
                break;
            }
        }
    }
}

fn worker_timestamp() -> Option<Timestamp> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    let millis = i64::try_from(elapsed.as_millis()).ok()?;
    Timestamp::from_unix_millis(millis).ok()
}
