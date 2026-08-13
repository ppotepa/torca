# FINALIZE — manual release runbook

This runbook is intentionally manual. The agent does not run builds, tests, or
deployments for this release gate.

## 1. Clean cold start

1. Connect the Windows host and the intended Android device.
2. Run the Rust wizard in `tools/torca-deploy` using a full reset with relay
   rebuild and preserved or rotated onion identity as desired.
3. Confirm the wizard reports each requested device as installed and launched.
4. Confirm both clients reach the local runtime shell without waiting for the
   relay. The network indicator may remain `connecting` while Tor/onion/relay
   recover independently.
5. Confirm `.torca/stack/relay_status.json` reports the active endpoint,
   publication state and `e2eVerified: true` before treating relay as ready.

## 2. Pairing

1. Generate an invitation on one device; verify placeholder, six-character
   code, QR and expiry timer appear in one modal.
2. Join using a code with and without spaces, then repeat with QR.
3. Accept the request on the creator; verify one modal/toast, one contact and
   one conversation on both devices.
4. Repeat reject, cancel, expiry and pairing again after removing a contact.

## 3. Messages and attachments

1. Exchange ordinary text in both directions and verify sent/delivered/read
   footer states and timestamps.
2. Queue an image, video and document. Verify preview, original filename,
   per-job progress, direction, retry and cancel on both sides.
3. Interrupt the relay or peer during a transfer, restore connectivity and
   verify the same attachment resumes without a duplicate message.
4. Export a completed attachment and verify the destination opens correctly.

## 4. Responsive and theme checks

1. On Windows, resize the window through narrow, normal and wide layouts.
2. On Android, verify the same flows use the narrow route and back navigation.
3. Check contact row chat/details actions, unread badges, jump-to-latest and
   the right-side details pane.
4. Check modern and terminal themes, keyboard focus, reduced motion and LED
   state colors.

## 5. Incident collection

1. Reproduce one failure and keep both clients running.
2. Run `scripts/collect.ps1 -Profile incident` from the repository root.
3. Verify the new incident folder contains relay status, relay logs, Windows
   runtime logs, Android runtime/logcat captures and a manifest with build IDs.
4. Confirm empty or stale files are reported as collection errors, not as a
   successful payload.
5. Archive the incident folder only after checking `file-inventory.json`.

## Acceptance evidence

Record the wizard run ID, endpoint, client build IDs, relay build/source
identity, device IDs and any incident folder path. A release is accepted only
when the same pairing and transfer scenarios pass on Windows and Android.
