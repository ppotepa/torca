# Torca current status

Last reviewed against `main`: **2026-08-25** (`a2ef0a7a29d41bee083aada3b99656d19d1f0780`).

Torca is security-sensitive **alpha** software. This page describes what is composed in the current source; it does not claim that every path has equivalent real-device validation or an independent security audit.

## Current product baseline

- Shared Flutter client for Windows and Android.
- Rust application/runtime owns durable state, security rules and networking.
- SQLCipher-backed structured storage plus platform-protected secret stores.
- One-to-one contacts, conversations, receipts, replies and searchable/paged history.
- Encrypted/resumable attachments.
- Explicit pairing and contact verification.
- Provider-neutral peer communication.
- Notifications, diagnostics and background/runtime policy.
- Mutual-consent half-duplex Radio Mode.
- Typed deployer and multi-process/device soak tooling.

Not in the supported baseline: groups, conventional calls, multi-device sync, public discovery, cloud backup or a supported Linux production client.

## Communication providers

| Provider | Deployment profile | Notes |
| --- | --- | --- |
| Tor | validated/selectable | default; managed onion rendezvous for pairing; onion peer transport; Radio supported |
| Iroh | validated/selectable | QUIC/direct path; direct QR/full-link bootstrap; Radio supported |
| WebRTC | hidden | adapter and native composition boundary exist; host session/signaling implementation is still the deployment blocker |
| Memory | hidden/test | deterministic/simulated runtime only |

`validated` above is the provider deployment-profile state in source. It means the provider can pass the normal composition/deploy gate; it is **not** an external security audit and does not imply identical privacy properties or identical device-soak evidence.

## Current maturity limits

- No independent production security audit is published for the project.
- The relationship-secret message design does not claim Signal-style forward secrecy or post-compromise security.
- Provider privacy differs: Tor uses onion routing; direct-path providers expose a different network-metadata surface.
- Availability remains subject to network/provider reachability, remote-peer availability and OS lifecycle/background limits.
- Alpha storage/protocol migrations should not be treated as long-term archival guarantees unless a release explicitly states otherwise.

## Validation model

The repository contains automated gates for:

- source architecture policy;
- Rust format/check/clippy/tests;
- runtime idle-scheduler/power regression tests;
- deploy planner dry-run smoke validation;
- generated Rust/Dart contract drift;
- Flutter formatting, analysis and tests; and
- Windows and Android debug builds.

The soak tool additionally supports runtime-lab, deterministic, active-messaging, idle-battery and connectivity scenarios, with Tor or Iroh as the selectable communication provider where the scenario supports it.

A workflow definition is not proof that the current branch passed CI. A source-level test is not evidence of real-device behavior. See [`testing.md`](testing.md) for evidence language.

## Most important remaining confidence work

Current risk is primarily confidence/evidence rather than missing core one-to-one product composition:

- repeat cross-platform pairing/messaging/attachment journeys for both selectable providers;
- longer Android lifecycle, network-loss/recovery and battery traces;
- Radio permission/background/recovery soak on supported provider/platform combinations;
- deployment interruption/resume and artifact-reuse testing; and
- independent review before stronger security claims.

## Canonical references

- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — current system ownership.
- [`transport.md`](transport.md) — provider model/status.
- [`app-flows.md`](app-flows.md) — current product/runtime flows.
- [`operations.md`](operations.md) — runtime/deployer behavior.
- [`testing.md`](testing.md) — validation gates.
- [`../SECURITY.md`](../SECURITY.md) and [`security/threat-model.md`](security/threat-model.md) — security boundaries and limits.

Historical plans/checklists have intentionally been removed from the maintained tree. Use Git history if an old implementation decision needs to be reconstructed.