impl TorcaRuntime {

fn begin_runtime_start(&mut self) {
    if self.is_closed() || self.host.is_some() || self.host_start.is_some() {
        return;
    }
    self.host_retry_at = None;
    self.host_state_hint = TorState::Starting;
    self.log(
        "bootstrap",
        Level::Info,
        "runtime",
        "TOR_STARTING",
        "Starting production network runtime",
    );
    let engine = self.application_runtime.handle().engine_handle();
    let (sender, receiver) = mpsc::channel::<HostStartEvent>();
    self.host_start = Some(receiver);
    self.host_start_started_at = Some(Instant::now());
    self.host_start_started_at_ms = unix_time_ms().ok();
    self.host_last_progress_at_ms = self.host_start_started_at_ms;
    self.host_progress = 0;
    self.host_attempt = 1;
    self.host_status_code = Some("TOR_BOOTSTRAP_STARTING".into());
    self.host_status_summary = Some("Starting embedded Tor bootstrap".into());
    self.host_onion_started_at_ms = None;
    self.host_onion_last_progress_at_ms = None;
    self.host_onion_progress = 0;
    self.host_onion_attempt = 0;
    self.host_onion_status_code = None;
    self.host_onion_status_summary = None;
    self.host_onion_retry_at = None;
    self.host_start_deadline = Some(Instant::now() + NETWORK_START_OBSERVE_TIMEOUT);
    let progress_sender = sender.clone();
    let observer: TorBootstrapObserver = std::sync::Arc::new(move |progress| {
        let _ = progress_sender.send(HostStartEvent::Progress(progress));
    });
    let read_receipt_policy = self.read_receipt_policy.clone();
    thread::Builder::new()
        .name("torca-network-start".into())
        .spawn(move || {
            let result = match catch_unwind(AssertUnwindSafe(|| {
                spawn_production_runtime(engine, observer, read_receipt_policy)
            })) {
                Ok(result) => result,
                Err(payload) => Err(NativeCompositionError::new(format!(
                    "production network runtime worker panicked: {}",
                    panic_message(payload)
                ))),
            };
            if let Err(send_error) = sender.send(HostStartEvent::Finished(result))
                && let HostStartEvent::Finished(Ok((_handle, owner, _radio))) = send_error.0
            {
                let _ = owner.shutdown();
            }
        })
        .expect("spawn Torca network startup worker");
}

fn create_bootstrap_identity(&mut self) -> Result<(), ()> {
    let identity_id_hex = crate::torca_runtime::secure_id_hex().map_err(|_| ())?;
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ())?;
    let at_ms = i64::try_from(elapsed.as_millis()).map_err(|_| ())?;
    let identity_id = identity_id_hex.parse::<OpaqueId>().map_err(|_| ())?;
    self.application_runtime.bootstrap_identity(identity_id, at_ms).map(|_| ()).map_err(|_| ())
}

fn apply_bootstrap_progress(&mut self, progress: &TorBootstrapEvent) -> bool {
    let retry_at = progress
        .retry_after_ms
        .and_then(|delay_ms| Instant::now().checked_add(Duration::from_millis(delay_ms)));
    match progress.stage {
        TorBootstrapStage::Network => {
            let changed = progress.progress != self.host_progress
                || self.host_status_code.as_deref() != Some(progress.code)
                || progress.attempt != self.host_attempt
                || self.host_status_summary.as_deref() != Some(progress.summary.as_str());
            if progress.progress > self.host_progress {
                self.host_last_progress_at_ms = unix_time_ms().ok();
            }
            self.host_progress = self.host_progress.max(progress.progress);
            self.host_attempt = progress.attempt;
            self.host_status_code = Some(progress.code.into());
            self.host_status_summary = Some(progress.summary.clone());
            self.host_retry_at = retry_at;
            changed
        }
        TorBootstrapStage::OnionService => {
            let changed = progress.progress != self.host_onion_progress
                || self.host_onion_status_code.as_deref() != Some(progress.code)
                || progress.attempt != self.host_onion_attempt
                || self.host_onion_status_summary.as_deref() != Some(progress.summary.as_str());
            let now_ms = unix_time_ms().ok();
            if self.host_onion_started_at_ms.is_none() {
                self.host_onion_started_at_ms = now_ms;
                self.host_onion_last_progress_at_ms = now_ms;
            }
            if progress.progress > self.host_onion_progress {
                self.host_onion_last_progress_at_ms = now_ms;
            }
            self.host_onion_progress = self.host_onion_progress.max(progress.progress);
            self.host_onion_attempt = progress.attempt;
            self.host_onion_status_code = Some(progress.code.into());
            self.host_onion_status_summary = Some(progress.summary.clone());
            self.host_onion_retry_at = retry_at;
            changed
        }
    }
}

#[allow(clippy::while_let_loop)]
fn advance_runtime_start(&mut self) {
    let mut outcome = None;
    loop {
        let event = match self.host_start.as_ref() {
            Some(receiver) => receiver.try_recv(),
            None => break,
        };
        match event {
            Ok(HostStartEvent::Progress(progress)) => {
                let changed = self.apply_bootstrap_progress(&progress);
                if changed {
                    self.log("tor", Level::Info, "bootstrap", progress.code, &progress.summary);
                }
            }
            Ok(HostStartEvent::Finished(result)) => {
                outcome = Some(result);
                break;
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                outcome = Some(Err(NativeCompositionError::new(
                    "network runtime startup worker disconnected",
                )));
                break;
            }
        }
    }
    if let Some(result) = outcome {
        self.host_start = None;
        self.host_start_started_at = None;
        self.host_start_deadline = None;
        match result {
            Ok((handle, owner, radio)) => {
                if self.network_changed_pending {
                    handle.network_changed();
                    self.network_changed_pending = false;
                }
                self.application_runtime.attach_runtime(handle);
                self.application_runtime.attach_radio(radio);
                self.apply_battery_policy(false);
                self.host = Some(owner);
                self.host_retry_at = None;
                self.host_failures = 0;
                self.host_state_hint = TorState::Ready;
                self.host_progress = 100;
                self.host_attempt = self.host_attempt.max(1);
                self.host_status_code = Some("TOR_BOOTSTRAP_READY".into());
                self.host_status_summary = Some("Tor network bootstrap completed".into());
                self.host_onion_progress = self.host_onion_progress.max(5);
                self.host_onion_status_code = Some("ONION_SERVICE_PUBLISHING".into());
                self.host_onion_status_summary =
                    Some("Waiting for private onion service reachability".into());
                self.log(
                    "bootstrap",
                    Level::Info,
                    "runtime",
                    "TOR_READY",
                    "Production network runtime is ready",
                );
            }
            Err(error) => {
                self.host_failures = self.host_failures.saturating_add(1);
                let retry_exhausted = self.host_failures >= NETWORK_MAX_ATTEMPTS;
                self.host_state_hint =
                    if retry_exhausted { TorState::Failed } else { TorState::Degraded };
                self.log(
                    "bootstrap",
                    Level::Error,
                    "runtime",
                    "RUNTIME_START_FAILED",
                    &format!("Production network runtime start failed: {error}"),
                );
                self.host_retry_at = (!retry_exhausted).then(|| {
                    let delay = match self.host_failures {
                        1 => Duration::from_secs(5),
                        2 => Duration::from_secs(15),
                        _ => NETWORK_RETRY_DELAY,
                    };
                    Instant::now() + delay
                });
                if self.host_retry_at.is_some() {
                    self.host_status_code = Some("TOR_BOOTSTRAP_RETRYING".into());
                    self.host_last_progress_at_ms = unix_time_ms().ok();
                } else {
                    self.host_status_code = Some("TOR_RUNTIME_FAILED".into());
                }
            }
        }
    } else if self.host_start.is_some()
        && self.host_start_deadline.is_some_and(|deadline| Instant::now() >= deadline)
    {
        self.host_start_deadline = None;
        self.log(
            "bootstrap",
            Level::Warn,
            "runtime",
            "RUNTIME_START_SLOW",
            "Production network runtime is still bootstrapping after 120 seconds",
        );
    }
    if self.host.is_none()
        && self.host_start.is_none()
        && self.host_retry_at.is_some_and(|deadline| Instant::now() >= deadline)
        && self.has_identity().unwrap_or(false)
    {
        self.begin_runtime_start();
    }
}

fn effective_battery_policy(&self, diagnostics_override: bool) -> EffectiveBatteryPolicy {
    self.battery_policy
        .effective(self.application_runtime.has_critical_network_lease(), diagnostics_override)
}

fn apply_battery_policy(&self, diagnostics_override: bool) {
    let effective = self.effective_battery_policy(diagnostics_override);
    self.application_runtime.set_battery_profile(effective.profile);
    self.application_runtime.set_background_sync(effective.background_sync);
    self.application_runtime.set_tor_dormancy(effective.tor_dormancy_allowed);
    self.application_runtime
        .set_metered_network(self.battery_policy.system.metered_network == Some(true));
    self.application_runtime.set_metered_transfer_policy(effective.metered_transfers);
}

fn has_identity(&self) -> Result<bool, ()> {
    self.application_runtime
        .handle()
        .overview()
        .map(|snapshot| snapshot.identity.is_some())
        .map_err(|_| ())
}

fn is_closed(&self) -> bool {
    self.actor.is_none()
}

fn log(&self, domain: &str, level: Level, component: &str, code: &str, message: &str) {
    if let Some(logger) = &self.logger {
        let _ = logger.event(domain, level, component, code, message);
    }
}

fn log_profile(&self, request_id: &str, code: &str) {
    if let Some(logger) = &self.logger {
        let context = json!({
            "requestId": request_id,
            "operation": "profile.set",
            "stage": code,
        })
        .to_string();
        let _ = logger.event_with_context(
            "profile",
            Level::Info,
            "profile",
            code,
            "profile operation stage",
            Some(&context),
        );
    }
}

fn log_pairing(
    &self,
    request_id: &str,
    operation: &str,
    session_id: &str,
    code: &str,
    outcome: Option<&str>,
) {
    if let Some(logger) = &self.logger {
        let context = json!({
            "requestId": request_id,
            "operation": operation,
            "sessionId": session_id,
            "outcome": outcome,
        })
        .to_string();
        let level = if code == "PAIRING_REQUEST_FAILED" { Level::Error } else { Level::Info };
        let _ = logger.event_with_context(
            "pairing",
            level,
            "command",
            code,
            "pairing operation stage",
            Some(&context),
        );
    }
}

}
