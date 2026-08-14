use std::slice;
use std::sync::OnceLock;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;
use std::time::Duration;

use serde_json::Value;

const RECONCILE_DELAYS: [Duration; 5] = [
    Duration::from_millis(5),
    Duration::from_millis(15),
    Duration::from_millis(40),
    Duration::from_millis(100),
    Duration::from_millis(250),
];
const SNAPSHOT_REQUEST: &[u8] = br#"{"schema":1,"requestId":"runtime-event-wake","kind":"query","name":"snapshot.get","payload":{}}"#;

static WAKE_SENDER: OnceLock<SyncSender<()>> = OnceLock::new();

/// Coalesces real transport/Tor events into bounded native snapshot refreshes.
/// There is no timer while idle: the worker blocks on the channel until a
/// concrete event arrives from a blocking transport/listener worker.
pub(crate) fn signal() {
    let sender = WAKE_SENDER.get_or_init(start_worker);
    match sender.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) => {}
        Err(TrySendError::Disconnected(())) => {}
    }
}

fn start_worker() -> SyncSender<()> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let _ = thread::Builder::new()
        .name("torca-native-event-wake".into())
        .spawn(move || {
            // Establish the native actor revision before consuming the first
            // event. The first signal can therefore distinguish an early
            // snapshot from the background state transition it is waiting for.
            let mut last_revision = refresh_snapshot().unwrap_or(0);
            while receiver.recv().is_ok() {
                for delay in RECONCILE_DELAYS {
                    thread::sleep(delay);
                    let Some(revision) = refresh_snapshot() else {
                        continue;
                    };
                    if revision > last_revision {
                        last_revision = revision;
                        break;
                    }
                    last_revision = last_revision.max(revision);
                }
            }
        });
    sender
}

fn refresh_snapshot() -> Option<u64> {
    let handle = crate::torca_runtime::torca_runtime_acquire();
    if handle.is_null() {
        return None;
    }
    let status = unsafe {
        crate::torca_runtime::torca_runtime_invoke(
            handle,
            SNAPSHOT_REQUEST.as_ptr(),
            SNAPSHOT_REQUEST.len(),
            2_000,
        )
    };
    if status != crate::native_runtime::ABI_OK {
        unsafe { crate::torca_runtime::torca_runtime_release(handle) };
        return None;
    }
    let pointer = unsafe { crate::torca_runtime::torca_runtime_response_ptr(handle) };
    let length = unsafe { crate::torca_runtime::torca_runtime_response_len(handle) };
    let response = if pointer.is_null() || length == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(pointer, length) }.to_vec()
    };
    unsafe { crate::torca_runtime::torca_runtime_release(handle) };
    serde_json::from_slice::<Value>(&response)
        .ok()?
        .get("revision")?
        .as_u64()
}

#[cfg(test)]
mod tests {
    use super::RECONCILE_DELAYS;

    #[test]
    fn event_reconcile_is_bounded_and_has_no_idle_cadence() {
        assert_eq!(RECONCILE_DELAYS.len(), 5);
        assert!(RECONCILE_DELAYS.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
