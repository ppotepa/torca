# Testing and validation

Torca uses layered evidence. A unit/integration test, a platform build, a host scenario and a real-device soak answer different questions. Change reports and release notes must name the evidence that actually exists.

The executable CI definition is `.github/workflows/validate.yml`; this document describes the stable validation model rather than duplicating every current job/flag.

## Evidence vocabulary

Use these terms precisely:

- **implemented** — the source path is present and composed.
- **source-validated** — named static/unit/integration checks were executed and passed.
- **platform-built** — a named target artifact was built successfully.
- **host-tested** — a scenario executed against local host processes/simulated peers.
- **device-validated** — a scenario executed on named physical/emulated device(s).
- **soak-validated** — a bounded long-running scenario completed with its stated verdict/evidence rules.
- **CI-green** — the referenced workflow run for the referenced commit completed successfully.
- **signed** — the referenced artifact was produced with the intended production signing identity/process.
- **audited** — only an actual independent security review justifies this term.

The existence of a test, script or workflow does not mean it passed for the current commit.

## Validation layers

### Rust/domain/application

Prefer focused deterministic tests for domain invariants, protocols, application state machines, retry/deadline behavior, provider-neutral ports, storage compatibility and negative/failure paths.

Workspace-level checks are appropriate before landing broad changes. The CI workflow remains the source of truth for the exact current command matrix.

### Generated contract and Flutter

Contract changes require generator drift checks plus affected Rust/Flutter tests. Flutter presentation changes require analysis/tests and platform execution when the behavior depends on real host lifecycle/permissions/window integration.

A Dart/widget test is not sufficient evidence for a Rust-owned durable/security state machine.

### Provider/integration

Provider changes should cover:

- native production composition;
- Iroh profile/build metadata;
- pairing/bootstrap lifecycle and invalid inputs;
- route freshness/generation behavior;
- peer transport connect/read/write/failure lifecycle;
- no silent fallback to a different provider; and
- provider conformance.

Memory is appropriate for deterministic tests; it is not a substitute for Iroh network evidence.

### Platform/device

Windows/Android behavior that depends on OS lifecycle, protected storage, notifications, permissions, network migration or background execution needs target-specific evidence. A successful cross-platform source test is not the same as a device run.

## Minimum evidence by change

| Change | Minimum deterministic evidence | Additional evidence when relevant |
| --- | --- | --- |
| domain/application logic | focused Rust tests + affected workspace checks | integration/soak for timing/network behavior |
| Flutter presentation | `flutter analyze` + focused/all Flutter tests | target platform run for host-specific behavior |
| generated/native contract | generator check + Rust/Flutter tests | Windows/Android build/runtime for ABI changes |
| storage/schema | repository/migration/compatibility tests | restart/upgrade profile scenario when installed data matters |
| Iroh/provider transport | provider tests + conformance + integration | real network/device reachability/migration scenario |
| pairing/security protocol | positive + negative protocol/application tests + threat-model review | cross-peer/device journey |
| Android lifecycle/background/permissions | affected source tests + Android build | emulator/physical Android validation |
| Windows lifecycle/tray | affected source tests + Windows build | Windows runtime validation |
| power/background policy | deterministic scheduler/governor tests | controlled physical-device power/lifecycle soak |

## Soak runner

`torca-soak` is the maintained validation orchestrator:

```powershell
cargo run -p torca-soak
```

Use its current `--help` output for exact scenarios/flags. Current scenarios cover deterministic/runtime labs, active messaging, idle battery and connectivity-style validation.

A soak report must identify at least the source/build, scenario, target/device, provider/profile, duration/repetitions and verdict. Interrupted/incomplete runs are not green.

## Power/battery evidence

CPU percentage is not battery energy. Power claims should control device/network/screen state, distinguish debug/release artifacts, use repeated measurements and retain enough diagnostics to explain app/runtime/provider activity.

For idle/background work the architectural expectation is event/deadline-driven behavior: no app-owned periodic polling/reconnect/database work should continue merely because the client is installed or a contact exists. If periodic activity appears, isolate its owner before changing arbitrary sleep intervals.

Physical Android measurements are required for Android battery claims; emulator CPU is useful diagnostic evidence but not calibrated energy usage.

## Security validation

Security-sensitive changes need negative/failure tests as well as happy paths. Bounded-input, fuzz/property testing can be useful around externally supplied pairing/wire/attachment inputs but does not replace design review or independent audit.

Review/update [`../SECURITY.md`](../SECURITY.md) and [`security/THREAT-MODEL.md`](security/THREAT-MODEL.md) when trust boundaries or claims change.

## Reporting results

Prefer exact reporting:

```text
Validated:
- <command/scenario and outcome>

Not run:
- <platform/device/soak/audit gate>
```

This is stronger than “tests pass” because it exposes both coverage and missing evidence.

## Dated evidence
