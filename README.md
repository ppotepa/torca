# Torca

Torca is a privacy-focused one-to-one messenger under active alpha development. One responsive Flutter client runs on Windows and Android and talks to a Rust application/runtime through a generated contract and native boundary. Rust owns identity, durable state, pairing, delivery, cryptography, storage, background coordination and communication policy.

Established contact traffic uses authenticated Iroh peer sessions with direct paths or Iroh relay fallback. Torca does not use a central message mailbox, and Iroh is not an anonymity network.

> **Status:** Torca is security-sensitive alpha software. It has not received an independent production security audit and is not yet a signed, production-ready high-risk communications product. Passing source tests, platform builds or local soaks are engineering evidence only; release and device claims require the corresponding executed evidence.

## Product shape

Current source includes:

- local installation identity and SQLCipher-backed durable state;
- platform-protected secret storage where supported;
- explicit pairing approval with QR/full-link commissioning;
- authenticated peer sessions plus application-layer authenticated encryption;
- durable outbound delivery, retry, delivered/read receipts and reply-to;
- paged/searchable conversation history;
- encrypted resumable attachments and explicit export/open flows;
- contact verification and identity-change protection;
- privacy-aware notifications and diagnostics;
- an experimental mutual-consent, half-duplex Radio Mode; and
- one shared Flutter application for Windows and Android.

Calls, groups, multi-device synchronization, public discovery and cloud backup are outside the current product scope. Linux is not a supported production client composition.

## Supported compositions

| Area | Current state |
| --- | --- |
| Windows client | supported alpha composition |
| Android client | supported alpha composition |
| Iroh | sole production communication provider |
| Memory provider | deterministic test double only |
| Tor | retired from the active product graph |
| WebRTC | unfinished adapter retired from the active product graph |

Iroh profiles affect reachability and network-metadata exposure, not Torca relationship identity or application-layer encryption. See [`docs/TRANSPORT.md`](docs/TRANSPORT.md).

## Architecture at a glance

```text
Flutter presentation
        |
EngineGateway + generated contract
        |
torca-native / platform boundary
        |
Rust application + process-owned runtime
        |
provider-neutral ports and durable repositories
        |
SQLCipher / crypto / files / Iroh transport
```

The dependency model is organized by architectural role:

```text
crates/foundation/       dependency-light primitives
crates/protocol/         bounded external/wire contracts
crates/domains/          product vocabulary and invariants
crates/application/      use cases, ports, runtime coordination and policy
crates/infrastructure/   storage, crypto, files and concrete provider adapters
crates/platform/         generated/native contract and OS composition
```

Flutter renders presentation-safe state and submits typed user intent. It is not a durable outbox, security-policy engine or second networking implementation. See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the maintained system model.

## Developer entry point

The canonical local build/run/deploy workflow is:

```powershell
cargo run -p torca-deploy
```

With no subcommand it opens the interactive deployment UI. The same planner/executor is available through CLI subcommands for repeatable automation. Use subcommand help rather than copying every option into documentation.

For UI-only work, run Flutter checks from `apps/client/flutter`. For validation expectations and evidence language, use [`docs/TESTING.md`](docs/TESTING.md).

## Repository map

```text
apps/client/flutter/      shared Windows/Android presentation client
crates/                   Rust product/runtime implementation
packages/                 reusable Flutter presentation packages
tests/torca-integration/  cross-crate integration journeys
tools/torca-deploy/       canonical build/run/deploy/log workflow
tools/torca-soak/         validation and soak orchestrator
scripts/                  policy, compatibility and measurement helpers
docs/                     maintained documentation and dated evidence
release/version.json      product/build/compatibility metadata
```

No active server implementation is maintained under `services/` in this checkout. Pairing-service protocol/client integration lives in the Rust workspace; established conversation traffic is peer-to-peer rather than routed through a Torca mailbox service.

## Documentation

Start with [`docs/README.md`](docs/README.md). The canonical documents are:

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system ownership, layering and dependency rules;
- [`docs/STATUS.md`](docs/STATUS.md) — current maturity, supported compositions and open release evidence;
- [`docs/APP-FLOWS.md`](docs/APP-FLOWS.md) — startup, pairing, messaging and background flows;
- [`docs/TRANSPORT.md`](docs/TRANSPORT.md) — provider-neutral transport boundary and Iroh behavior;
- [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) — local development workflow;
- [`docs/TESTING.md`](docs/TESTING.md) — validation layers and evidence language;
- [`docs/OPERATIONS.md`](docs/OPERATIONS.md) — deploy/runtime diagnostics and recovery;
- [`SECURITY.md`](SECURITY.md), [`docs/security/THREAT-MODEL.md`](docs/security/THREAT-MODEL.md) and [`PRIVACY.md`](PRIVACY.md) — security/privacy boundaries;
- [`docs/VERSIONING-AND-RELEASES.md`](docs/VERSIONING-AND-RELEASES.md) — product/build/compatibility version rules; and
- [`CHANGELOG.md`](CHANGELOG.md) — user/developer-visible changes from this documentation baseline forward.


## Version and release metadata

The current product version, build number, release channel and compatibility markers are declared in [`release/version.json`](release/version.json). Rust workspace and Flutter package metadata mirror the product version for packaging. Do not infer release readiness from a version string alone.

## Security and privacy

Torca does not claim Signal-style forward secrecy or post-compromise security in the current relationship-key design. A compromised endpoint can access plaintext and keys; recipients can copy delivered content; direct network paths can expose network-location metadata; availability remains best effort under suspension, network failure and denial of service.

Read [`SECURITY.md`](SECURITY.md) and [`docs/security/THREAT-MODEL.md`](docs/security/THREAT-MODEL.md) before changing pairing, peer authentication, cryptography, storage, notifications, Radio Mode, diagnostics or platform/privacy boundaries.

## License

Torca is licensed under AGPL-3.0-or-later. See [`LICENSE`](LICENSE) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
