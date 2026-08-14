# Diagnostic logging and incident collection

Torca keeps runtime producers separate and creates a fresh incident snapshot when diagnostics are collected. Diagnostics are operational evidence, not an alternative source of product state, and must remain redaction-conscious.

## Canonical collection

From the repository root, use the Rust deployment tool:

```powershell
cargo run -p torca-deploy -- logs --target all
```

The `logs` command uses the same device discovery/planning path as normal deployment work. Each run creates a new incident directory under the Torca runtime state instead of reusing one mutable `logs/` folder.

Do not document the legacy ZIP collector/inventory format as the canonical path unless the Rust collector is deliberately changed to produce it again.

## Current incident contents

The Rust collector writes a `manifest.json` when collection starts and then captures the evidence that is available for the selected environment.

The current layout is conceptually:

```text
<incident>/
  manifest.json
  deploy-state.json                 # when current deploy state exists
  relay/
    relay.log                       # when Docker relay logs are available
    state/
      relay_endpoint.txt            # when present
      relay_ready.txt               # when present
      relay_status.json             # when present
  windows/
    native/...                      # copied from the Windows Torca log root
  android-<device>/
    native/...                      # copied app-native logs when accessible
    logcat.log                      # bounded process Logcat capture when available
```

A manifest proves that collection started. It does **not** prove that useful incident evidence was captured. The collector considers a collection payload-bearing only when at least one non-empty file beyond `manifest.json` exists.

## Producer locations

### Windows

The native client logger writes below:

```text
%LOCALAPPDATA%\Torca\logs
```

The collector copies the available tree into the incident's `windows/native` directory.

### Android

The collector first attempts to enumerate Torca native log files under the app's scoped external files area and copies matching `.log`/`.json` files. It also captures a bounded Logcat snapshot, scoped to the Torca process when a PID can be resolved.

Android collection depends on ADB/device access and platform storage visibility. Missing evidence must not be described as a complete incident merely because a manifest exists.

### Relay

When the repository Docker Compose file is available, the collector captures a bounded tail of the live `relay` service output. It also copies the current endpoint/readiness/status files when present.

Relay output and endpoint state can contain operational metadata. Do not add invitation codes, relationship secrets, private keys or message/radio payloads to relay diagnostics.

## Redaction rules

Diagnostics should prefer typed states, timestamps, counters, bounded identifiers and redacted error descriptors. Do not log:

- message or attachment plaintext;
- Radio Mode audio payloads;
- private identity keys or pairwise relationship secrets;
- database/storage encryption keys;
- pairing invitation capabilities/tickets; or
- authentication material that could recreate a peer relationship.

Onion endpoints and device/build identifiers can still be sensitive operational metadata. Share incident directories only with the people who need them.

## Interpreting a collection

When using diagnostics as validation evidence, record:

- the incident directory path;
- selected/discovered devices;
- client and relay build/source identity where available;
- the active relay endpoint/status;
- the scenario and approximate timestamp; and
- which expected producer files were actually present.

A partial capture can still be useful for debugging, but describe it as partial rather than implying all producers were collected.

## Related documents

- [`STATUS.md`](STATUS.md) — current validation/maturity summary.
- [`FINALIZE_MANUAL_RUNBOOK.md`](FINALIZE_MANUAL_RUNBOOK.md) — manual Windows/Android acceptance procedure.
- [`../SECURITY.md`](../SECURITY.md) — observability/security requirements.
- [`../PRIVACY.md`](../PRIVACY.md) — user-facing diagnostics/privacy behavior.