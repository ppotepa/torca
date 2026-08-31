# Runtime operations and diagnostics

This page describes the maintained build/deploy/runtime operational model. It is not a release checklist; exact run evidence belongs to the artifacts/logs that produced it.

## Deployment owner

`torca-deploy` is the canonical planner/executor for local build, install, launch, logs, reset policy and resume:

```powershell
cargo run -p torca-deploy
```

Representative automation:

```powershell
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --target all --configuration debug
cargo run -p torca-deploy -- build --target windows --configuration debug
cargo run -p torca-deploy -- deploy --target android --device <adb-serial>
cargo run -p torca-deploy -- logs --target all
cargo run -p torca-deploy -- resume
```

Use command help for exact flags. PowerShell helpers remain policy/compatibility/measurement utilities rather than a second deployment architecture.

## Deploy checkpoints

Deployment state is written below `.torca/deploy/`, including the current checkpoint and per-run event history. Checkpoints include plan/build identity so interrupted work can be deliberately resumed and stale/incompatible plans rejected.

A built artifact, discovered device or partially completed step is not proof that a deployment succeeded.

## Release/artifact identity

[`../release/version.json`](../release/version.json) declares product/build/channel and compatibility metadata used by packaging/manifests. Deployment manifests project this identity together with target/configuration/provider/profile and artifact hashes/build metadata.

Artifact reuse must fail on incompatible provider/profile/build metadata rather than silently deploying the wrong native/client pair. See [`versioning-and-releases.md`](versioning-and-releases.md).

## Production provider

Iroh is the sole production communication provider; Memory is test-only. Iroh endpoint identity/provider route state is owned by native/infrastructure composition.

The application distinguishes local readiness from provider reachability. Temporary network/provider degradation must not make usable encrypted local state unavailable.

## Startup readiness

Flutter opens the FFI gateway, initializes the Rust application/runtime and reports `flutter_gateway_ready` after it has decoded a successful initial application response. Platform host integrations attach after this boundary.

A provider can still be degraded after local readiness. Health UIs and deployment diagnostics should report those states separately.

## Installed data compatibility

Structured client data uses SQLCipher-backed storage. Startup validates the storage epoch before normal migrations. An incompatible epoch is rejected explicitly rather than silently interpreted as current data.

If an installed profile is incompatible, follow the migration/reset policy for that change. Do not workaround the compatibility guard by manually editing metadata or deleting isolated tables.

## Lifecycle and background work

Windows and Android feed lifecycle state into the same process-owned Rust runtime. Screens, notification callbacks and platform services must not create independent runtimes.

Runtime work is driven by durable demand, provider/platform events and deadlines. Idle UI polling must not be required for retry, pairing or incoming work.

Battery/background policy and Iroh reachability profile are separate concepts. A lower-reachability profile can reduce network work but must not be used to hide an application hot loop.

## Diagnostics

The native/application boundary exposes structured diagnostics and bounded log-tail collection. Useful diagnostics can include:

- source/build/product/provider/profile identity;
- provider/route/connection health;
- durable queue/retry state and bounded timing counters;
- runtime wake/deadline/power observations;
- lifecycle/background observations; and
- redacted errors needed to explain a run.

Diagnostics must not intentionally include message/attachment plaintext, Radio audio, private identity keys, database keys, relationship secrets or reusable invitation capabilities. Treat collected bundles as potentially sensitive operational artifacts anyway.

## Common failure classes

| Symptom | Operational response |
| --- | --- |
| native symbol/procedure/ABI mismatch | deploy a matching Flutter/native artifact set; do not retry indefinitely against an incompatible library |
| provider/network degraded while local data opens | inspect Iroh/profile/route diagnostics without resetting local identity/history |
| pending message after transient network failure | preserve durable state; inspect delivery/provider health because retry ownership is Rust-side |
| pairing fails/expires | preserve first failure evidence; inspect bounded invitation/route/provider state rather than repeatedly recreating identities |
| incompatible storage epoch | use the documented migration/reset decision for that version; do not bypass the guard |
| unexplained CPU/battery activity | isolate Flutter/runtime/Iroh/thread/wake owner with soak/measurement tooling before adding sleeps or wider polling intervals |

## Android privacy and signing

Android screen capture is blocked by default. The explicit development `--privacy allow-capture` option changes the OS secure-window behavior only; it does not change Torca encryption or Iroh transport privacy. Screenshots/logs produced in this mode can contain sensitive test data.

The current Android release Gradle configuration uses debug signing. It is suitable for development iteration only and must be replaced by a production signing/provenance process before public release.

## Reset and destructive actions

Client reset is deliberate and can remove identity, encrypted history, relationship/provider route state and cached secrets. Ordinary rebuild/deploy work should preserve data unless the test explicitly requires a clean profile.

Do not reset repeatedly during incident capture: it destroys the evidence needed to diagnose retry/storage/provider lifecycle failures.

## Soak and incident artifacts

Soak/measurement tooling writes ignored local evidence under `.torca/`. A useful incident/soak record captures:

1. exact source commit/build/product/provider/profile/platform/device;
2. scenario and relevant lifecycle/network conditions;
3. first relevant failure and bounded redacted diagnostics;
4. whether the peer/device was simulated, emulator-based or physical; and
5. the final verdict, including incomplete/aborted state.

Dated reports promoted into `docs/validation/` must preserve this context and remain historical snapshots.

## Recovery principles

- unsent durable work remains Rust-owned;
- provider failure does not transfer retry ownership to Flutter;
- stale provider/host state is generation-bounded and recoverable;
- pairing sessions expire/fail explicitly rather than pretending unavailable bootstrap is healthy;
- local encrypted state remains usable whenever local initialization succeeds; and
- deploy checkpoints support deliberate recovery after interrupted host/device operations.

See [`testing.md`](testing.md) for evidence terms and [`transport.md`](transport.md) for provider behavior.
