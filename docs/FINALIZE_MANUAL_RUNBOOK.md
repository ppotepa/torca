# Torca 0.3 manual acceptance runbook

This runbook covers real Windows/Android acceptance evidence that cannot be replaced by source checks alone. It is intentionally manual and should be executed on the exact artifacts being evaluated.

The current implementation and validation status lives in [`STATUS.md`](STATUS.md).
This runbook records manual evidence; it does not turn unchecked notes into
completed release gates.

## Before the run

1. Confirm the repository commit/branch being evaluated.
2. Connect the intended Android device and identify the Windows host.
3. Check the current deployment state:

   ```powershell
   cargo run -p torca-deploy -- status
   ```

4. Use the Rust wizard for the coordinated deploy:

   ```powershell
   cargo run -p torca-deploy
   ```

5. Choose the reset/rebuild/onion policy required by the acceptance scenario. Preserve the onion identity unless rotation is part of the test. Treat client-data reset and onion rotation as destructive actions.
6. Keep Android screen-capture privacy in the default strict mode unless a screenshot is specifically required for local test evidence and the test operator deliberately enables capture.

Record the resulting deploy run/checkpoint identity, source commit, client build IDs, relay identity/endpoint and device IDs.

## 1. Startup and local availability

1. Launch Windows and Android from the coordinated deploy.
2. Confirm both clients reach usable local UI without waiting for relay availability.
3. Confirm Tor/onion/relay progress can remain degraded/connecting without hiding local encrypted history or settings.
4. Confirm the active relay status corresponds to the expected endpoint before treating pairing service reachability as ready.
5. Restart each client once and verify local profile/history remains available.

## 2. Pairing and contact lifecycle

Exercise both code and QR paths where supported.

1. Generate an invitation and verify one clear presentation of code, QR and expiry.
2. Join from the other device using the six-character code, including normalized spacing/hyphen input where supported.
3. Accept on the creator and verify exactly one contact/conversation is created on both devices.
4. Verify the remote display name and Safety Number/security projection are available.
5. Repeat reject, cancel and expiry flows.
6. Remove a contact and verify the intended re-pairing policy works without duplicate durable state.
7. Temporarily make the relay unavailable during a pairing operation; restore it and verify the client recovers without requiring an application restart.

## 3. Text messaging and receipts

1. Exchange text in both directions.
2. Verify durable local queueing while the peer/network is unavailable.
3. Restore connectivity and confirm queued work is delivered without duplicate messages.
4. Verify sent/delivered/read state and timestamps.
5. Exercise reply-to, retry and conversation paging/search on a non-empty history.
6. Restart one client with pending work and verify retry/recovery continues correctly.

## 4. Attachments

1. Queue representative image, video and document files within supported limits.
2. Verify filename/type/size presentation, transfer direction and progress.
3. Interrupt connectivity during transfer, restore it and verify the same attachment resumes without creating a duplicate message/job.
4. Cancel a transfer and verify temporary state is cleaned without affecting unrelated attachments.
5. Retry a failed transfer.
6. Export/open a completed attachment and verify the resulting file is correct.
7. Repeat one transfer across a client restart to validate durable resume behavior.

## 5. Radio Mode

Radio Mode is experimental and requires explicit acceptance evidence before stronger readiness claims.

1. Enable Radio for the same contact on both devices and confirm the session reaches ready only after mutual consent.
2. Deny Android microphone permission and verify capture does not start; grant permission and retry without restarting the application.
3. Hold PTT from Windows to Android, then Android to Windows; verify only the floor holder transmits and release stops the burst.
4. Verify the configured burst limit ends transmission even if input remains held.
5. Background/foreground the Android app during a session and verify the session recovers or terminates visibly/safely.
6. Break the Tor/network route during a Radio session and verify one bounded interrupted/reconnecting path rather than concurrent sessions or reconnect storms.
7. Disable Radio on either side and verify media closes and durable consent/session state is updated as designed.

## 6. Network and recovery behavior

1. Switch Android between Wi-Fi/cellular or otherwise trigger a default-network change.
2. Verify stale routes are invalidated and recovery occurs without a reconnect storm.
3. Restart/repair the relay while established contacts exist; normal contact history must remain locally available.
4. Verify established peer messaging is not redefined as relay-dependent.
5. Exercise a cold/warm Tor startup comparison without deleting state between tests unless the scenario explicitly requires it.

## 7. Responsive UI and privacy

1. Resize Windows through narrow, normal and wide layouts.
2. Verify Android narrow navigation/back behavior for the same flows.
3. Check keyboard/touch focus, unread indicators, jump-to-latest and contact/chat/details actions.
4. Check modern and terminal appearance variants plus reduced-motion behavior.
5. Verify Android blocks screenshots/screen recording in strict mode. If capture is explicitly enabled for testing, verify returning to strict mode restores the protection.
6. Verify notification privacy settings do not disclose content beyond the selected policy.

## 8. Incident collection

After reproducing at least one failure/recovery scenario, collect a fresh incident snapshot:

```powershell
cargo run -p torca-deploy -- logs --target all
```

Verify the new incident directory contains useful non-empty payload beyond `manifest.json`, and inspect the available relay state/logs, Windows native logs and per-device Android native/logcat evidence. Record missing producers as missing/partial evidence instead of calling the collection complete.

See [`diagnostics.md`](diagnostics.md) for the current collector layout.

## Acceptance record

For each completed run, record:

- source commit and working tree/branch identity;
- deployment run/checkpoint ID;
- Windows and Android build IDs;
- relay build/source identity and endpoint;
- device/OS identifiers needed to reproduce the environment;
- scenarios completed and scenarios skipped/failed;
- incident directory paths for failures; and
- any manual privacy override used during evidence capture.

A source-complete feature is not automatically device-validated. Do not mark the release/device gate complete until the relevant Windows ↔ Android scenarios above have actually passed on the evaluated artifacts.
