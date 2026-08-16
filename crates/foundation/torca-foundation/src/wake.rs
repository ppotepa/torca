use std::sync::{Arc, Mutex};

/// Cloneable callback type used only as a non-blocking wake edge.
pub type WakeCallback = Arc<dyn Fn() + Send + Sync>;

/// Small callback slot shared by background workers.
///
/// The callback is cloned while the mutex is held and always invoked after the
/// lock has been released, preventing re-entrant wake paths from deadlocking.
#[derive(Default)]
pub struct WakeSlot {
    callback: Mutex<Option<WakeCallback>>,
}

impl WakeSlot {
    pub fn set(&self, callback: WakeCallback) -> bool {
        self.callback
            .lock()
            .map(|mut slot| {
                *slot = Some(callback);
                true
            })
            .unwrap_or(false)
    }

    pub fn clear(&self) -> bool {
        self.callback
            .lock()
            .map(|mut slot| {
                *slot = None;
                true
            })
            .unwrap_or(false)
    }

    pub fn wake(&self) -> bool {
        let callback = self.callback.lock().ok().and_then(|slot| slot.clone());
        let Some(callback) = callback else {
            return false;
        };
        callback();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::WakeSlot;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn wake_invokes_the_current_callback() {
        let slot = WakeSlot::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        assert!(slot.set(Arc::new(move || {
            observed.fetch_add(1, Ordering::Release);
        })));
        assert!(slot.wake());
        assert_eq!(calls.load(Ordering::Acquire), 1);
        assert!(slot.clear());
        assert!(!slot.wake());
    }
}
