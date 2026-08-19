#[unsafe(no_mangle)]
pub extern "C" fn torca_runtime_metadata_ptr() -> *const u8 {
    metadata().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_runtime_metadata_len() -> usize {
    metadata().len()
}

fn metadata() -> &'static [u8] {
    METADATA.get_or_init(|| {
        serde_json::to_vec(&json!({
            "productVersion": PRODUCT_VERSION,
            "buildNumber": 1,
            "buildId": BUILD_ID,
            "sourceCommit": SOURCE_COMMIT,
            "sourceFingerprint": SOURCE_FINGERPRINT,
            "nativeAbi": NATIVE_ABI,
            "contractSchema": CONTRACT_VERSION,
            "storageEpoch": STORAGE_EPOCH,
            "schemaVersion": 1,
            "wireVersion": 1,
            "relayEndpointHash": RELAY_ENDPOINT_HASH,
            "targetPlatform": std::env::consts::OS,
            "targetArchitecture": std::env::consts::ARCH,
            "capabilities": {
                "maxAttachmentBytes": torca_attachments::MAX_ATTACHMENT_BYTES,
                "maxVideoAttachmentBytes": 5 * 1024 * 1024,
                "maxQueuedAttachments": 5,
                "maxAttachmentSourceBytes": 64 * 1024 * 1024,
            },
        }))
        .expect("static runtime metadata is serializable")
    })
}

#[cfg(target_os = "android")]
pub(crate) fn notification_snapshot_json(after_cursor: u64) -> String {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    let inner = match registry.lock().ok().and_then(|guard| guard.as_ref().cloned()) {
        Some(inner) => inner,
        None => return crate::notification_json::notification_events_json(after_cursor),
    };
    let request = torca_contract::notification_poll_request_json(
        &format!("android-notifications-{after_cursor}"),
        after_cursor,
    );
    let (tx, rx) = mpsc::sync_channel(1);
    if send_with_timeout(
        &inner.sender,
        ActorMessage::Invoke { request, response: tx },
        Duration::from_secs(2),
    )
    .is_err()
    {
        return crate::notification_json::notification_events_json(after_cursor);
    }
    let Ok(response) = rx.recv_timeout(DEFAULT_QUERY_TIMEOUT) else {
        return crate::notification_json::notification_events_json(after_cursor);
    };
    extract_notification_snapshot(&response).map_or_else(
        || crate::notification_json::notification_events_json(after_cursor),
        |value| value.to_string(),
    )
}

#[cfg(any(target_os = "android", test))]
fn extract_notification_snapshot(response: &[u8]) -> Option<Value> {
    let response = serde_json::from_slice::<Value>(response).ok()?;
    let runtime_id = response.get("runtimeId")?.as_str()?.to_owned();
    let mut snapshot = response.get("snapshot")?.clone();
    snapshot.as_object_mut()?.insert("runtimeId".into(), Value::String(runtime_id));
    Some(snapshot)
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_alloc(length: usize) -> *mut u8 {
    if length == 0 {
        return ptr::null_mut();
    }
    Box::into_raw(vec![0_u8; length].into_boxed_slice()).cast::<u8>()
}

#[unsafe(no_mangle)]
/// # Safety
/// `data` must be a pointer and length previously returned by this library.
pub unsafe extern "C" fn torca_free(data: *mut u8, length: usize) {
    if data.is_null() || length == 0 {
        return;
    }
    let raw = ptr::slice_from_raw_parts_mut(data, length);
    unsafe {
        drop(Box::from_raw(raw));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_runtime_acquire() -> *mut TorcaRuntimeHandle {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    let existing = match registry.lock() {
        Ok(mut guard) => {
            if guard.as_ref().is_some_and(|inner| !inner.alive.load(Ordering::Acquire)) {
                // The actor exited but a stale registry entry may still be held by
                // platform waiters. Drop it so the next acquire performs a clean
                // controlled respawn instead of returning a zombie handle.
                *guard = None;
            }
            guard.as_ref().cloned()
        }
        Err(_) => return ptr::null_mut(),
    };
    let inner = if let Some(existing) = existing {
        existing
    } else {
        // Never hold the registry mutex while constructing storage, identity or
        // the network runtime. A separate initialization gate serializes the
        // slow path, then the registry is acquired only for the short publish.
        let initialization = INITIALIZATION_LOCK.get_or_init(|| Mutex::new(()));
        let Ok(_initialization_guard) = initialization.lock() else {
            return ptr::null_mut();
        };
        if let Ok(guard) = registry.lock() {
            if let Some(existing) = guard
                .as_ref()
                .filter(|inner| inner.alive.load(Ordering::Acquire))
                .cloned()
            {
                existing
            } else {
                drop(guard);
                let Ok(value) = spawn_runtime() else {
                    return ptr::null_mut();
                };
                let Ok(mut guard) = registry.lock() else {
                    return ptr::null_mut();
                };
                if let Some(existing) = guard.as_ref().cloned() {
                    existing
                } else {
                    *guard = Some(Arc::clone(&value));
                    value
                }
            }
        } else {
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(TorcaRuntimeHandle { inner, response: Mutex::new(Vec::new()) }))
}

fn spawn_runtime() -> Result<Arc<RuntimeHandleInner>, ()> {
    let (sender, receiver) = mpsc::sync_channel(MAILBOX_CAPACITY);
    let event_hub = Arc::new(RuntimeEventHub::default());
    let actor_event_hub = Arc::clone(&event_hub);
    let alive = Arc::new(AtomicBool::new(false));
    let actor_alive = Arc::clone(&alive);
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
    thread::Builder::new()
        .name("torca-runtime".into())
        .spawn(move || match TorcaRuntime::new(Arc::clone(&actor_event_hub)) {
            Ok(runtime) => {
                actor_alive.store(true, Ordering::Release);
                let runtime_id = secure_id_hex().unwrap_or_else(|_| "runtime-unavailable".into());
                let _ = ready_tx.send(Ok(()));
                actor_loop(
                    receiver,
                    ActorState {
                        runtime,
                        runtime_id,
                        revision: 1,
                        completed: IdempotencyLedger::default(),
                    },
                    actor_event_hub,
                );
                actor_alive.store(false, Ordering::Release);
            }
            Err(error) => {
                eprintln!("Torca runtime startup failed: {error}");
                actor_alive.store(false, Ordering::Release);
                let _ = ready_tx.send(Err(error));
            }
        })
        .map_err(|_| ())?;
    let startup_error = match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => return Err(()),
    };
    Ok(Arc::new(RuntimeHandleInner { sender, startup_error, event_hub, alive }))
}

fn actor_loop(
    receiver: Receiver<ActorMessage>,
    mut state: ActorState,
    event_hub: Arc<RuntimeEventHub>,
) {
    loop {
        let message = match state.next_maintenance_delay() {
            Some(timeout) => match receiver.recv_timeout(timeout) {
                Ok(message) => message,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if state.maintain() {
                        state.revision = state.revision.saturating_add(1);
                        event_hub.publish(state.revision);
                    }
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            },
            None => match receiver.recv() {
                Ok(message) => message,
                Err(_) => break,
            },
        };
        match message {
            ActorMessage::Invoke { request, response } => {
                let emits_revision = request_emits_runtime_revision(&request);
                let _ = response.send(state.invoke(&request));
                if emits_revision {
                    state.revision = state.revision.saturating_add(1);
                    event_hub.publish(state.revision);
                }
                if state.maintain() {
                    state.revision = state.revision.saturating_add(1);
                    event_hub.publish(state.revision);
                }
            }
            ActorMessage::Lifecycle { event, response } => {
                let _ = response.send(state.runtime.lifecycle(&event));
                state.revision = state.revision.saturating_add(1);
                if state.maintain() {
                    state.revision = state.revision.saturating_add(1);
                    event_hub.publish(state.revision);
                }
                event_hub.publish(state.revision);
            }
            ActorMessage::Shutdown { response } => {
                let _ = state.runtime.close();
                let _ = response.send(());
                break;
            }
        }
    }
}

fn request_emits_runtime_revision(raw: &str) -> bool {
    let Ok(request) = serde_json::from_str::<Value>(raw) else {
        return true;
    };
    request.get("kind").and_then(Value::as_str) != Some("query")
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn dispatch_lifecycle(event: &str) -> i32 {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    let Some(inner) = registry.lock().ok().and_then(|guard| guard.as_ref().cloned()) else {
        return -1;
    };
    let (tx, rx) = mpsc::sync_channel(1);
    if send_with_timeout(
        &inner.sender,
        ActorMessage::Lifecycle { event: event.to_owned(), response: tx },
        Duration::from_secs(2),
    )
    .is_err()
    {
        return -1;
    }
    rx.recv_timeout(DEFAULT_QUERY_TIMEOUT).unwrap_or(-1)
}
