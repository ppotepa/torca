#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_has_compatibility_fields() {
        let value: Value = serde_json::from_slice(metadata()).expect("valid metadata");
        assert_eq!(value["metadataSchema"], 2);
        assert!(value["communicationProvider"].is_string());
        assert!(value.get("providerEndpointHash").is_some());
        assert!(value.get("relayEndpointHash").is_some());
        assert_eq!(value["nativeAbi"], NATIVE_ABI);
        assert_eq!(value["storageEpoch"], STORAGE_EPOCH);
        assert_eq!(value["contractSchema"], CONTRACT_VERSION);
        assert_eq!(
            value["capabilities"]["maxAttachmentBytes"],
            torca_attachments::MAX_ATTACHMENT_BYTES
        );
        assert_eq!(value["capabilities"]["maxQueuedAttachments"], 5);
        assert!(value["capabilities"]["pairingQr"].is_boolean());
        assert!(value["capabilities"]["pairingFullLink"].is_boolean());
        assert!(value["capabilities"]["pairingShortCode"].is_boolean());
        assert!(value["capabilities"]["supportsRadio"].is_boolean());
        assert!(value["buildId"].is_string());
        assert!(value["sourceFingerprint"].is_string());
    }

    #[test]
    fn notification_snapshot_carries_the_process_runtime_identifier() {
        let response = br#"{
            "runtimeId":"runtime-a",
            "snapshot":{"afterCursor":4,"events":[]}
        }"#;
        let snapshot = extract_notification_snapshot(response).expect("notification snapshot");
        assert_eq!(snapshot["runtimeId"], "runtime-a");
        assert_eq!(snapshot["afterCursor"], 4);
    }

    #[test]
    fn command_ledger_is_bounded_and_expires_entries() {
        let now = Instant::now();
        let mut ledger = IdempotencyLedger::with_limits(2, Duration::from_secs(10));
        ledger.insert("a".into(), b"a".to_vec(), now);
        ledger.insert("b".into(), b"b".to_vec(), now);
        ledger.insert("c".into(), b"c".to_vec(), now);
        assert!(ledger.get("a", now).is_none());
        assert_eq!(ledger.get("b", now), Some(b"b".to_vec()));
        assert_eq!(ledger.get("c", now), Some(b"c".to_vec()));
        assert!(ledger.get("b", now + Duration::from_secs(11)).is_none());
    }

    #[test]
    fn query_request_ids_are_not_command_ledger_entries() {
        let now = Instant::now();
        let mut ledger = IdempotencyLedger::default();
        assert!(is_idempotent_command("command"));
        assert!(!is_idempotent_command("query"));
        assert!(!is_idempotent_command("lifecycle"));
        assert!(ledger.get("notifications-poll-10", now).is_none());
        ledger.insert("command-1".into(), b"command".to_vec(), now);
        assert!(ledger.get("notifications-poll-10", now).is_none());
    }

    #[test]
    fn queries_do_not_count_as_revision_transitions() {
        assert!(operation_counts_for_revision("query", "snapshot.get"));
        assert!(!operation_counts_for_revision("query", "notifications.poll"));
        assert!(!operation_counts_for_revision("query", "conversation.page"));
        assert!(operation_counts_for_revision("command", "profile.set"));
        assert!(operation_counts_for_revision("lifecycle", "foregrounded"));
    }

    #[test]
    fn queries_do_not_wake_revision_waiters_without_state_change() {
        assert!(!request_emits_runtime_revision(
            r#"{"kind":"query","name":"runtime.poll"}"#,
        ));
        assert!(!request_emits_runtime_revision(
            r#"{"kind":"query","name":"snapshot.get"}"#,
        ));
        assert!(request_emits_runtime_revision(
            r#"{"kind":"command","name":"message.send"}"#,
        ));
        assert!(request_emits_runtime_revision("not-json"));
    }
}
