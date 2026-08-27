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
        let provider = torca_transport_api::TransportKind::from_wire(COMMUNICATION_PROVIDER)
            .unwrap_or_default();
        let profile = provider.deployment_profile();
        // The selected Iroh profile is immutable build input. Exposing its
        // canonical value makes mixed-profile installs diagnosable without
        // leaking endpoint material into the application contract.
        let provider_profile = if provider == torca_transport_api::TransportKind::Iroh {
            Some(match IROH_PROFILE.unwrap_or("always") {
                "direct" | "direct-only" => "direct",
                "local" | "local-only" => "local",
                _ => "always",
            })
        } else {
            None
        };
        let features = profile.features;
        // Endpoint hashes identify commissioning services, not direct peer
        // transports. Ignore an inherited/stale build variable for Iroh,
        // Memory, and other providers without such a service.
        let provider_endpoint_hash = profile
            .commissioning_service
            .requires_endpoint()
            .then_some(PROVIDER_ENDPOINT_HASH)
            .flatten();
        serde_json::to_vec(&json!({
            "metadataSchema": 2,
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
            "communicationProvider": COMMUNICATION_PROVIDER,
            "providerProfile": provider_profile,
            "providerEndpointRequired": profile.commissioning_service.requires_endpoint(),
            "providerEndpointHash": provider_endpoint_hash,
            // Temporary compatibility alias for older Flutter binaries. It
            // is intentionally nullable for direct providers such as Iroh.
            "relayEndpointHash": provider_endpoint_hash,
            "targetPlatform": std::env::consts::OS,
            "targetArchitecture": std::env::consts::ARCH,
            "capabilities": {
                "maxAttachmentBytes": torca_attachments::MAX_ATTACHMENT_BYTES,
                "maxVideoAttachmentBytes": 5 * 1024 * 1024,
                "maxQueuedAttachments": 5,
                "maxAttachmentSourceBytes": 64 * 1024 * 1024,
                // A short code only works when the selected provider has a
                // discovery/rendezvous mapping. Direct Iroh QR currently does
                // not, so advertising this as true would recreate the
                // terminal-code-as-retry bug in the UI.
                "pairingQr": features.pairing_qr,
                "pairingFullLink": features.pairing_full_link,
                "pairingShortCode": features.pairing_short_code,
                "supportsIncoming": features.incoming,
                "supportsRadio": features.radio,
                "supportsAttachments": features.attachments,
                "providerDirectPath": features.direct_path,
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
    let sender_for_actor = sender.clone();
    thread::Builder::new()
        .name("torca-runtime".into())
        .spawn(move || match TorcaRuntime::new(Arc::clone(&actor_event_hub)) {
            Ok(mut runtime) => {
                let actor_sender = sender_for_actor.clone();
                let actor_wake_pending = Arc::new(AtomicBool::new(false));
                runtime.actor_wake_pending = Some(Arc::clone(&actor_wake_pending));
                runtime.actor_waker = Some(Arc::new(move || {
                    if actor_wake_pending.swap(true, Ordering::AcqRel) {
                        return;
                    }
                    if actor_sender.try_send(ActorMessage::InternalWake).is_err() {
                        actor_wake_pending.store(false, Ordering::Release);
                    }
                }));
                actor_alive.store(true, Ordering::Release);
                let runtime_id = secure_id_hex().unwrap_or_else(|_| "runtime-unavailable".into());
                let _ = ready_tx.send(Ok(()));
                let mut state = ActorState {
                    runtime,
                    runtime_id,
                    revision: 1,
                    completed: IdempotencyLedger::default(),
                };
                let dead_event_hub = Arc::clone(&actor_event_hub);
                let actor_result = catch_unwind(AssertUnwindSafe(|| {
                    actor_loop(receiver, &mut state, actor_event_hub);
                }));
                if let Err(payload) = actor_result {
                    let detail = if let Some(message) = payload.downcast_ref::<&str>() {
                        (*message).to_owned()
                    } else if let Some(message) = payload.downcast_ref::<String>() {
                        message.clone()
                    } else {
                        "non-string actor panic payload".to_owned()
                    };
                    state.runtime.log(
                        "runtime",
                        Level::Error,
                        "actor",
                        "RUNTIME_ACTOR_PANICKED",
                        &detail,
                    );
                    eprintln!("Torca runtime actor panicked: {detail}");
                }
                // A blocking revision waiter may have passed the initial
                // `alive` check immediately before the actor stopped.  Wake
                // those waiters explicitly; otherwise Android's foreground
                // service (and Flutter's event isolate) can remain blocked
                // forever on a dead runtime and never get a chance to
                // reacquire a fresh generation.
                dead_event_hub.close();
                actor_alive.store(false, Ordering::Release);
            }
            Err(error) => {
                eprintln!("Torca runtime startup failed: {error}");
                actor_alive.store(false, Ordering::Release);
                actor_event_hub.close();
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
    state: &mut ActorState,
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
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    state.runtime.log(
                        "runtime",
                        Level::Warn,
                        "actor",
                        "RUNTIME_ACTOR_STOPPED",
                        "actor mailbox disconnected",
                    );
                    break;
                }
            },
            None => match receiver.recv() {
                Ok(message) => message,
                Err(_) => {
                    state.runtime.log(
                        "runtime",
                        Level::Warn,
                        "actor",
                        "RUNTIME_ACTOR_STOPPED",
                        "actor mailbox disconnected",
                    );
                    break;
                }
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
            ActorMessage::InternalWake => {
                if let Some(gate) = state.runtime.actor_wake_pending.as_ref() {
                    gate.store(false, Ordering::Release);
                }
                if state.maintain() {
                    state.revision = state.revision.saturating_add(1);
                    event_hub.publish(state.revision);
                }
            }
            ActorMessage::Shutdown { response, source } => {
                state.runtime.log(
                    "ffi",
                    Level::Info,
                    "shutdown",
                    "RUNTIME_SHUTDOWN_REQUESTED",
                    source,
                );
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
