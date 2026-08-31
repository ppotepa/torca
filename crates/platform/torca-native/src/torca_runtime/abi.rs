#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be a valid handle returned by `torca_runtime_acquire` and
/// must not be used after this call.
pub unsafe extern "C" fn torca_runtime_release(handle: *mut TorcaRuntimeHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be valid and `request` must reference `request_length` readable bytes.
pub unsafe extern "C" fn torca_runtime_invoke(
    handle: *mut TorcaRuntimeHandle,
    request: *const u8,
    request_length: usize,
    timeout_ms: u32,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    if request.is_null() || request_length == 0 {
        return -1;
    }
    let Ok(request) = str::from_utf8(unsafe { slice::from_raw_parts(request, request_length) })
    else {
        return -1;
    };
    if handle.inner.startup_error.is_some() || !handle.inner.alive.load(Ordering::Acquire) {
        let request_id = serde_json::from_str::<Value>(request)
            .ok()
            .and_then(|value| value.get("requestId").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        let storage_epoch_incompatible = handle
            .inner
            .startup_error
            .as_deref()
            .is_some_and(|error| error.contains("INCOMPATIBLE_STORAGE_EPOCH"));
        let (code, category, retryable, message_key) = if storage_epoch_incompatible {
            (
                "INCOMPATIBLE_STORAGE_EPOCH",
                "storage",
                false,
                "storage.epoch.incompatible",
            )
        } else {
            ("RUNTIME_STARTUP_FAILED", "runtime", true, "runtime.startup.failed")
        };
        // Keep the ABI response useful when the actor cannot start.  The old
        // response only contained a localization key, which made every
        // provider failure look identical in Flutter ("runtime not ready")
        // and forced us to guess from logcat.  Composition errors are already
        // redacted at their source; expose the bounded diagnostic only in
        // debug builds so release builds retain the generic contract.
        let diagnostic = if storage_epoch_incompatible {
            "installed storage is incompatible; explicit reset required".to_owned()
        } else if cfg!(debug_assertions) {
            handle
                .inner
                .startup_error
                .as_deref()
                .unwrap_or("runtime actor is no longer alive")
                .chars()
                .take(512)
                .collect::<String>()
        } else {
            "runtime initialization failed".to_owned()
        };
        let response = serde_json::to_vec(&json!({
            "schema": 1,
            "requestId": request_id,
            "status": "failed",
            "resultKind": "error",
            "runtimeId": "runtime-unavailable",
            "revision": 0,
            "snapshot": Value::Null,
            "error": {
                "code": code,
                "category": category,
                "severity": "error",
                "retryable": retryable,
                "messageKey": message_key,
                "message": diagnostic,
                "resetRequired": storage_epoch_incompatible,
                "diagnosticId": secure_id_hex().unwrap_or_default()
            },
            "timing": { "queuedMs": 0, "executionMs": 0 }
        }))
        .unwrap_or_else(|_| b"{\"status\":\"failed\"}".to_vec());
        if let Ok(mut target) = handle.response.lock() {
            *target = response;
            return ABI_OK;
        }
        return -1;
    }
    let (tx, rx) = mpsc::sync_channel(1);
    if send_with_timeout(
        &handle.inner.sender,
        ActorMessage::Invoke { request: request.to_owned(), response: tx },
        Duration::from_secs(2),
    )
    .is_err()
    {
        return -2;
    }
    let timeout = if timeout_ms == 0 {
        DEFAULT_QUERY_TIMEOUT
    } else {
        Duration::from_millis(u64::from(timeout_ms))
    };
    let Ok(response) = rx.recv_timeout(timeout) else {
        return -2;
    };
    let Ok(mut target) = handle.response.lock() else {
        return -1;
    };
    *target = response;
    ABI_OK
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be a valid handle returned by `torca_runtime_acquire`.
pub unsafe extern "C" fn torca_runtime_wait_for_revision(
    handle: *const TorcaRuntimeHandle,
    after_revision: u64,
    after_cursor: u64,
    timeout_ms: u32,
) -> i32 {
    unsafe {
        torca_runtime_wait_for_revision_with_waiter(
            handle,
            after_revision,
            after_cursor,
            timeout_ms,
            1,
        )
    }
}

unsafe fn torca_runtime_wait_for_revision_with_waiter(
    handle: *const TorcaRuntimeHandle,
    after_revision: u64,
    after_cursor: u64,
    timeout_ms: u32,
    waiter: u64,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    if handle.inner.startup_error.is_some() || !handle.inner.alive.load(Ordering::Acquire) {
        return -2;
    }
    let result = if timeout_ms == 0 {
        handle.inner.event_hub.wait_indefinitely(waiter, after_revision, after_cursor)
    } else {
        handle.inner.event_hub.wait(
            waiter,
            after_revision,
            after_cursor,
            Duration::from_millis(u64::from(timeout_ms)),
        )
    };
    if handle.inner.event_hub.is_closed() {
        // A waiter may have passed the initial liveness check just before the
        // actor terminated.  Return the same failure code as an unavailable
        // runtime so platform hosts can reacquire a new generation instead
        // of treating the shutdown wake as a normal application revision.
        return -2;
    }
    match result {
        Some(_) => 1,
        None => 0,
    }
}

#[cfg(target_os = "android")]
unsafe fn torca_runtime_wait_for_runtime_revision_with_waiter(
    handle: *const TorcaRuntimeHandle,
    after_revision: u64,
    timeout_ms: u32,
    waiter: u64,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    if handle.inner.startup_error.is_some() || !handle.inner.alive.load(Ordering::Acquire) {
        return -2;
    }
    let result = handle.inner.event_hub.wait_revision(
        waiter,
        after_revision,
        if timeout_ms == 0 {
            Duration::from_secs(365 * 24 * 60 * 60)
        } else {
            Duration::from_millis(u64::from(timeout_ms))
        },
    );
    if handle.inner.event_hub.is_closed() {
        return -2;
    }
    if result.is_some() { 1 } else { 0 }
}

#[cfg(target_os = "android")]
unsafe fn torca_runtime_wait_for_notification_with_waiter(
    handle: *const TorcaRuntimeHandle,
    after_cursor: u64,
    timeout_ms: u32,
    waiter: u64,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    if handle.inner.startup_error.is_some() || !handle.inner.alive.load(Ordering::Acquire) {
        return -2;
    }
    let result = handle.inner.event_hub.wait_notification(
        waiter,
        after_cursor,
        if timeout_ms == 0 {
            Duration::from_secs(365 * 24 * 60 * 60)
        } else {
            Duration::from_millis(u64::from(timeout_ms))
        },
    );
    if handle.inner.event_hub.is_closed() {
        return -2;
    }
    if result.is_some() { 1 } else { 0 }
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be a valid handle returned by `torca_runtime_acquire`.
pub unsafe extern "C" fn torca_runtime_cancel_revision_wait(
    handle: *const TorcaRuntimeHandle,
) -> i32 {
    unsafe { torca_runtime_cancel_revision_wait_for(handle, 1) }
}

unsafe fn torca_runtime_cancel_revision_wait_for(
    handle: *const TorcaRuntimeHandle,
    waiter: u64,
) -> i32 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    handle.inner.event_hub.cancel(waiter);
    ABI_OK
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be a valid runtime handle and the returned pointer is valid
/// until the next invocation on that handle.
pub unsafe extern "C" fn torca_runtime_response_ptr(
    handle: *const TorcaRuntimeHandle,
) -> *const u8 {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return ptr::null();
    };
    handle.response.lock().map_or(ptr::null(), |value| value.as_ptr())
}

#[unsafe(no_mangle)]
/// # Safety
/// `handle` must be a valid runtime handle.
pub unsafe extern "C" fn torca_runtime_response_len(handle: *const TorcaRuntimeHandle) -> usize {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return 0;
    };
    handle.response.lock().map_or(0, |value| value.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn torca_runtime_shutdown(timeout_ms: u32) -> i32 {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    let inner = match registry.lock() {
        Ok(mut value) => value.take(),
        Err(_) => return -1,
    };
    let Some(inner) = inner else {
        return ABI_OK;
    };
    if inner.startup_error.is_some() {
        return ABI_OK;
    }
    let (tx, rx) = mpsc::sync_channel(1);
    if send_with_timeout(
        &inner.sender,
        ActorMessage::Shutdown { response: tx, source: "abi.shutdown" },
        Duration::from_secs(2),
    )
    .is_err()
    {
        return -2;
    }
    let timeout = if timeout_ms == 0 {
        SHUTDOWN_TIMEOUT
    } else {
        Duration::from_millis(u64::from(timeout_ms))
    };
    if rx.recv_timeout(timeout).is_ok() { ABI_OK } else { -2 }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeEnsureRuntime(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) -> jni::sys::jboolean {
    let handle = torca_runtime_acquire();
    if handle.is_null() {
        0
    } else {
        let available =
            unsafe {
                handle.as_ref().is_some_and(|value| {
                    value.inner.startup_error.is_none()
                        && value.inner.alive.load(Ordering::Acquire)
                })
            };
        unsafe {
            torca_runtime_release(handle);
        }
        u8::from(available)
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeRuntimeAvailable(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) -> jni::sys::jboolean {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    registry.lock().map_or(0, |guard| {
        u8::from(guard.as_ref().is_some_and(|value| {
            value.startup_error.is_none() && value.alive.load(Ordering::Acquire)
        }))
    })
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeLifecycleEvent(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    event: jni::sys::jstring,
) -> jni::sys::jboolean {
    let Ok(mut env) = (unsafe { jni::JNIEnv::from_raw(env) }) else { return 0 };
    let event = unsafe { jni::objects::JString::from_raw(event) };
    let Ok(event) = env.get_string(&event) else { return 0 };
    u8::from(dispatch_lifecycle(event.to_string_lossy().as_ref()) == ABI_OK)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeNotificationSnapshotJson(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    after_cursor: jni::sys::jlong,
) -> jni::sys::jstring {
    let payload = notification_snapshot_json(after_cursor.max(0) as u64);
    let Ok(env) = (unsafe { jni::JNIEnv::from_raw(env) }) else {
        return core::ptr::null_mut();
    };
    env.new_string(payload).map_or(core::ptr::null_mut(), |value| value.into_raw())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeRuntimeRevision(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) -> jni::sys::jlong {
    let registry = REGISTRY.get_or_init(|| Mutex::new(None));
    registry
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().and_then(|inner| inner.event_hub.current()))
        .map_or(0, |(revision, _)| revision as jni::sys::jlong)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeWaitForRevision(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    after_revision: jni::sys::jlong,
    after_cursor: jni::sys::jlong,
    timeout_ms: jni::sys::jint,
) -> jni::sys::jint {
    let handle = torca_runtime_acquire();
    if handle.is_null() {
        return -1;
    }
    let result = unsafe {
        torca_runtime_wait_for_revision_with_waiter(
            handle,
            after_revision.max(0) as u64,
            after_cursor.max(0) as u64,
            timeout_ms.max(0) as u32,
            2,
        )
    };
    unsafe { torca_runtime_release(handle) };
    result
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeWaitForRuntimeRevision(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    after_revision: jni::sys::jlong,
    timeout_ms: jni::sys::jint,
) -> jni::sys::jint {
    let handle = torca_runtime_acquire();
    if handle.is_null() {
        return -1;
    }
    let result = unsafe {
        torca_runtime_wait_for_runtime_revision_with_waiter(
            handle,
            after_revision.max(0) as u64,
            timeout_ms.max(0) as u32,
            2,
        )
    };
    unsafe { torca_runtime_release(handle) };
    result
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeWaitForNotification(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
    after_cursor: jni::sys::jlong,
    timeout_ms: jni::sys::jint,
) -> jni::sys::jint {
    let handle = torca_runtime_acquire();
    if handle.is_null() {
        return -1;
    }
    let result = unsafe {
        torca_runtime_wait_for_notification_with_waiter(
            handle,
            after_cursor.max(0) as u64,
            timeout_ms.max(0) as u32,
            2,
        )
    };
    unsafe { torca_runtime_release(handle) };
    result
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_torca_host_NativeRuntimeBridge_nativeCancelRevisionWait(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jclass,
) -> jni::sys::jint {
    let handle = torca_runtime_acquire();
    if handle.is_null() {
        return -1;
    }
    let result = unsafe { torca_runtime_cancel_revision_wait_for(handle, 2) };
    unsafe { torca_runtime_release(handle) };
    result
}
