impl ActorState {
    fn next_maintenance_delay(&self) -> Option<Duration> {
        self.runtime.next_pending_operation_delay()
    }

    fn maintain(&mut self) -> bool {
        let Some(delay) = self.runtime.next_pending_operation_delay() else {
            return false;
        };
        if !delay.is_zero() {
            return false;
        }
        // This is the sole reconciliation path for pending work. It also
        // refreshes the read model so background completions can wake event
        // consumers without a foreground query.
        let before = self.runtime.snapshot_json.clone();
        self.runtime.reconcile_pending_operations();
        before != self.runtime.snapshot_json
    }

    fn invoke(&mut self, raw: &str) -> Vec<u8> {
        let started = Instant::now();
        let request: Value = match serde_json::from_str(raw) {
            Ok(value) => value,
            Err(_) => {
                return self.error(
                    "",
                    "CONTRACT_REQUEST_INVALID",
                    "contract.request.invalid",
                    false,
                );
            }
        };
        let request_id = request.get("requestId").and_then(Value::as_str).unwrap_or_default();
        if request.get("schema").and_then(Value::as_u64) != Some(1) {
            return self.error(
                request_id,
                "CONTRACT_SCHEMA_MISMATCH",
                "contract.schema.mismatch",
                false,
            );
        }
        let name = request.get("name").and_then(Value::as_str).unwrap_or_default();
        let payload = request.get("payload").cloned().unwrap_or_else(|| json!({}));
        let kind = request.get("kind").and_then(Value::as_str).unwrap_or_default();
        if !generated::contains(kind, name) {
            return self.error(
                request_id,
                "CONTRACT_OPERATION_UNKNOWN",
                "contract.operation.unknown",
                false,
            );
        }
        if is_idempotent_command(kind)
            && !request_id.is_empty()
            && let Some(response) = self.completed.get(request_id, Instant::now())
        {
            return response;
        }
        let before_snapshot = self.runtime.snapshot_json.clone();
        let before_notification_cursor = self.runtime.notification_cursor;
        let code = match (kind, name) {
            ("query", "snapshot.get") => self.runtime.refresh_snapshot(),
            ("query", "conversation.page") => {
                let conversation =
                    payload.get("conversationId").and_then(Value::as_str).unwrap_or_default();
                let before =
                    payload.get("beforeMessageId").and_then(Value::as_str).unwrap_or_default();
                let before_at_ms = payload.get("beforeAtMs").and_then(Value::as_i64);
                let limit =
                    payload.get("limit").and_then(Value::as_u64).unwrap_or(100).clamp(1, 200)
                        as u32;
                let cursor = if before.is_empty() { None } else { Some(before) };
                self.runtime.conversation_page(conversation, before_at_ms, cursor, limit as usize)
            }
            ("query", "conversation.search") => {
                let conversation =
                    payload.get("conversationId").and_then(Value::as_str).unwrap_or_default();
                let query = payload.get("query").and_then(Value::as_str).unwrap_or_default();
                let limit =
                    payload.get("limit").and_then(Value::as_u64).unwrap_or(100).clamp(1, 200)
                        as u32;
                self.runtime.search_messages(conversation, query, limit as usize)
            }
            ("query", "notifications.poll") => {
                let cursor = payload.get("afterCursor").and_then(Value::as_u64).unwrap_or(0);
                self.runtime.notification_events_json(cursor)
            }
            ("query", "runtime.poll") => {
                let cursor = payload.get("afterCursor").and_then(Value::as_u64).unwrap_or(0);
                let snapshot_code = self.runtime.refresh_snapshot();
                if snapshot_code != ABI_OK {
                    snapshot_code
                } else {
                    let events_code = self.runtime.notification_events_json(cursor);
                    if events_code == ABI_OK {
                        let snapshot = serde_json::from_str::<Value>(&self.runtime.snapshot_json)
                            .unwrap_or(Value::Null);
                        let events = serde_json::from_str::<Value>(&self.runtime.query_json)
                            .unwrap_or(Value::Null);
                        self.runtime.query_json = serde_json::json!({
                            "snapshot": snapshot,
                            "events": events.get("events").cloned().unwrap_or_else(|| serde_json::json!([])),
                            "afterCursor": events.get("afterCursor").cloned().unwrap_or(Value::from(cursor)),
                        }).to_string();
                    }
                    events_code
                }
            }
            ("query", "diagnostics.get") => self.runtime.diagnostics_json(),
            ("query", "avatars.get") => {
                self.runtime.avatar_genome_json(payload.get("identityId").and_then(Value::as_str))
            }
            ("query", "pairing.parse") => {
                let uri = payload.get("uri").and_then(Value::as_str).unwrap_or_default();
                self.runtime.parse_pairing_uri(uri)
            }
            ("query", "pairing.encode") => {
                let code = payload.get("code").and_then(Value::as_str).unwrap_or_default();
                self.runtime.encode_pairing_uri(code)
            }
            ("command", _) => {
                let command = match bridge_command(name, &payload) {
                    Ok(command) => command,
                    Err((code, key)) => return self.error(request_id, code, key, false),
                };
                self.runtime.execute_with_request_id(command, request_id)
            }
            ("lifecycle", event) => self.runtime.lifecycle(event),
            _ => {
                return self.error(
                    request_id,
                    "CONTRACT_OPERATION_UNKNOWN",
                    "contract.operation.unknown",
                    false,
                );
            }
        };
        if code != ABI_OK {
            return self.native_error(request_id);
        }
        let counts_for_revision = operation_counts_for_revision(kind, name);
        let state_changed = counts_for_revision
            && (self.runtime.snapshot_json != before_snapshot
                || self.runtime.notification_cursor != before_notification_cursor);
        if state_changed {
            self.revision = self.revision.saturating_add(1);
        }
        let mut snapshot: Value = if name == "conversation.page"
            || name == "conversation.search"
            || name == "notifications.poll"
            || name == "runtime.poll"
            || name == "diagnostics.get"
            || name == "pairing.parse"
            || name == "pairing.encode"
        {
            serde_json::from_str(&self.runtime.query_json).unwrap_or(Value::Null)
        } else {
            serde_json::from_str(&self.runtime.snapshot_json).unwrap_or(Value::Null)
        };
        if name != "conversation.page"
            && name != "conversation.search"
            && name != "notifications.poll"
            && name != "runtime.poll"
            && name != "diagnostics.get"
            && let Value::Object(object) = &mut snapshot
        {
            object.insert("runtimeId".into(), Value::String(self.runtime_id.clone()));
            object.insert("revision".into(), Value::from(self.revision));
            object
                .insert("notificationCursor".into(), Value::from(self.runtime.notification_cursor));
        }
        let operation_result =
            serde_json::from_str::<Value>(&self.runtime.last_result_json).unwrap_or(Value::Null);
        let result_kind = operation_result
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or(if name == "profile.set" { "profile_updated" } else { "snapshot" });
        let resource_id = operation_result.get("resourceId").cloned().unwrap_or(Value::Null);
        let invite_uri = operation_result.get("inviteUri").cloned().unwrap_or(Value::Null);
        let response = serde_json::to_vec(&json!({
            "schema": 1, "requestId": request_id, "status": "succeeded",
            "resultKind": result_kind,
            "resourceId": resource_id,
            "inviteUri": invite_uri,
            "runtimeId": self.runtime_id, "revision": self.revision, "snapshot": snapshot,
            "error": Value::Null, "timing": { "queuedMs": 0, "executionMs": started.elapsed().as_millis() }
        })).expect("runtime response is serializable");
        if is_idempotent_command(kind) && !request_id.is_empty() {
            self.completed.insert(request_id.to_owned(), response.clone(), Instant::now());
        }
        response
    }

    fn error(&self, request_id: &str, code: &str, message_key: &str, retryable: bool) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema": 1, "requestId": request_id, "status": "failed", "resultKind": "error",
            "runtimeId": self.runtime_id, "revision": self.revision, "snapshot": Value::Null,
            "error": { "code": code, "category": "runtime", "severity": "error",
                "retryable": retryable, "messageKey": message_key, "diagnosticId": secure_id_hex().unwrap_or_default() },
            "timing": { "queuedMs": 0, "executionMs": 0 }
        })).expect("runtime error is serializable")
    }

    fn native_error(&self, request_id: &str) -> Vec<u8> {
        let descriptor = serde_json::from_str::<Value>(&self.runtime.last_result_json)
            .ok()
            .and_then(|value| {
                let kind = value.get("kind")?.as_str()?.strip_prefix("error:")?.to_owned();
                Some(kind)
            })
            .unwrap_or_else(|| "RUNTIME_OPERATION_FAILED".into());
        let normalized = descriptor.to_ascii_uppercase();
        let message_key = match normalized.as_str() {
            "PROFILE_NOT_READY" => "profile.not_ready",
            "PROFILE_SNAPSHOT_INCONSISTENT" => "profile.snapshot.inconsistent",
            "RELAY_DEGRADED" => "relay.degraded",
            "RELAY_NOT_READY" => "relay.not_ready",
            "IDENTITY_CHANGED" => "identity.changed",
            _ => "runtime.operation.failed",
        };
        self.error(request_id, &normalized, message_key, normalized != "PROFILE_NOT_READY")
    }
}
