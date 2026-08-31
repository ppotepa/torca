# Testing and validation

Torca uses layered evidence. A source test, a platform build and a real-device soak answer different questions; documentation and pull requests should name exactly which evidence exists.

## CI baseline

`.github/workflows/validate.yml` currently defines four jobs.

### Rust core

```powershell
./scripts/modules/Torca.SourcePolicy.ps1 -RepoRoot .
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::correctness -D clippy::suspicious -D clippy::perf
cargo test --workspace --all-targets --all-features --locked
cargo test --locked -p torca-runtime idle_scheduler_has_no_application_deadline
cargo test --locked -p torca-runtime-policy idle_governor_has_no_scheduled_work_or_active_leases
cargo run --locked -p torca-deploy -- full-redeploy --target all --configuration debug --dry-run
```

### Flutter/bridge contract

From/against `apps/client/flutter` the workflow resolves packages, checks Dart formatting, verifies generated contract drift, runs `flutter analyze` and `flutter test`.

Contract drift check:

```powershell
cargo run -p torca-contract-gen -- --check apps/client/flutter/lib/generated/torca_contract.dart
```

### Platform builds

After core/Flutter gates pass, CI builds:

- Windows debug client; and
- Android debug client.

The build jobs validate the Iroh-only production composition. Provider
conformance and device soak are separate evidence.

## Evidence levels

Use these terms precisely:

- **implemented** — source path is present/composed.
- **source-validated** — named static/unit/integration gates were executed and passed.
- **platform-built** — a named platform artifact was built successfully.
- **host-tested** — scenario executed against local process/simulated peers.
- **device-validated** — scenario executed on named physical/emulated device(s).
- **soak-validated** — bounded long-running scenario completed with its verdict/evidence requirements.
- **CI-green** — the referenced workflow run for the referenced commit completed successfully.
- **audited** — only an actual independent security review may justify this term.

Do not turn the existence of a test or workflow into a passing result.

## Flutter coverage areas

The Flutter test suite includes current surfaces around pairing, conversations/history, attachments/capabilities, diagnostics/runtime status, read receipts, voice clips/Radio-related widgets, transfer UI, navigation, theming and platform behavior abstractions.

When changing a durable workflow, pair widget/unit tests with Rust/application coverage; a Dart test alone should not become the proof for a Rust-owned state machine.

## Rust/integration coverage

The Rust workspace includes unit/contract tests across domain, protocol, application, transport, infrastructure and platform composition plus `tests/torca-integration` for cross-crate journeys.

Provider changes should test:

- deployment profile/manifest behavior;
- native provider selection/composition;
- commissioning lifecycle;
- pairing bootstrap and invalid-input behavior;
- peer transport framing/connection lifecycle;
- capability projection; and
- no silent fallback to another provider.

## Soak cockpit

`torca-soak` is the current orchestrator. With no arguments it can open the interactive workflow; CLI flags support automation.

```powershell
cargo run -p torca-soak
```

Current scenarios include:

- `runtime-lab` — multi-process production-runtime lab with isolated fake-peer profiles;
- `deterministic` — repeated deterministic Rust suite;
- `active-messaging` — Android plus fake peers exchanging real messages through the production runtime;
- `idle-battery` — physical Android idle/battery measurement; and
- `connectivity` — Android network loss/recovery loop.

The soak CLI exercises Iroh profiles and the Memory test double where a
deterministic fixture is appropriate. Direct Iroh reachability and relay
fallback are separate runtime evidence, not anonymity guarantees.

Examples should be derived from `cargo run -p torca-soak -- --help` when exact flags matter. Do not keep a second hand-maintained SOAK1/SOAK2/battery checklist as the source of truth.

## What to run for common changes

| Change | Minimum deterministic evidence | Additional evidence when behavior is platform/network sensitive |
| --- | --- | --- |
| domain/application logic | focused Rust tests + relevant workspace checks | integration/soak if timing/networked |
| Flutter presentation | `flutter analyze`, focused/all Flutter tests | target platform run for host behavior |
| generated contract | generator check + Rust/Flutter tests | target builds if ABI/native behavior changes |
| provider transport | provider tests + application/runtime integration | real/provider network scenario |
| pairing/security protocol | negative/protocol/application tests + threat-model review | cross-peer/device journey |
| Android lifecycle/notifications/permissions | relevant Rust/Flutter tests + Android build | physical/emulated Android validation |
| Windows lifecycle/tray | relevant Rust/Flutter tests + Windows build | Windows runtime validation |
| power/background policy | runtime policy tests | timed device battery/lifecycle soak |

## Pull-request reporting

A useful validation section states commands and outcomes, for example:

```text
Validated:
- cargo test -p torca-transport-iroh
- cargo test -p torca-integration <focused-test>
- flutter analyze

Not run:
- Windows build
- Android/Iroh real-device soak
```

This is stronger than “tests pass” because reviewers can distinguish coverage from missing evidence.

## Security validation

Security-sensitive changes require negative/failure tests as well as happy paths. Fuzzing/property/bounded-input tests are appropriate around externally supplied wire/pairing/attachment data, but their presence still does not replace independent review.

See [`../SECURITY.md`](../SECURITY.md) and [`security/threat-model.md`](security/threat-model.md).
