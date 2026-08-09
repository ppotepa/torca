# Torca

Torca is a privacy-focused 1:1 messenger built around local identities, Tor onion services, encrypted local storage and explicit contact pairing. Windows and Android use the same responsive Flutter client and the same Rust runtime; platform code is limited to operating-system integration.

The previous [`ppotepa/tOrca`](https://github.com/ppotepa/tOrca) repository is a requirements/reference source, not a second active implementation.

## Current engineering state

The active code line is the unified baseline. Source checks are green; platform/release validation is still open. Start with [`0.2_PROGRESS.md`](0.2_PROGRESS.md); do not infer release readiness from source completion.

The baseline provides a reliable daily-use 1:1 messenger:

- local installation identity and encrypted SQLCipher persistence;
- short-lived pairing codes/QR with explicit approval;
- direct authenticated peer delivery through Tor onion services;
- durable message retry, delivered/read receipts and reply-to;
- paged/searchable conversation history and conversation summaries;
- encrypted/resumable attachments;
- per-contact PeerHealth and redacted diagnostics;
- local Safety Number verification with identity-change send blocking;
- notification privacy and host-level screen-capture protection;
- one shared responsive Flutter application for Windows and Android.

Calls, groups, multi-device sync, public discovery, cloud backup and Linux production composition are outside this baseline.

## Security scope

Torca authenticates peers and encrypts peer payloads with a protected pairwise secret established during pairing. It does **not** currently implement MLS or a Double Ratchet-style per-message key schedule, so forward secrecy and post-compromise security are not claimed for message history. See [`SECURITY.md`](SECURITY.md) and [`docs/security/threat-model.md`](docs/security/threat-model.md).

## Architecture

```text
responsive Flutter UI
        |
EngineGateway / generated DTOs
        |
torca-native C ABI
        |
process-owned TorcaRuntime actor
        |
SQLCipher / crypto / peer link / embedded torca-tor
```

Important rules:

- Flutter renders state and submits typed user intent; Rust owns identifiers, timestamps, durable state, networking and security rules.
- Business SQL lives in parameterized `.sql` files owned by storage crates.
- Pairing may use the untrusted ephemeral relay; normal contact traffic is direct over Tor.
- Production never silently falls back to the memory gateway.
- Long-running network startup must not prevent access to local encrypted history.
- Normal UI snapshots do not load the complete message history; conversation history uses bounded SQLCipher paging/search.

## Repository layout

```text
apps/client/flutter/      single responsive application client
crates/foundation/        dependency-light primitives
crates/domains/           domain vocabulary and invariants
crates/application/       engine/runtime/application orchestration
crates/infrastructure/    SQLCipher, crypto, files, peer and Tor adapters
crates/protocol/          wire/pairing/relay/peer protocols
crates/platform/          bridge, native ABI and platform adapters
services/relay/           ephemeral pairing rendezvous broker
tests/torca-integration/  cross-crate integration journeys
scripts/modules/          private build/source-policy/platform implementation
tools/build/overlays/     required platform templates
docs/0.2/                 current source track and final audit
```

## Developer workflow

Public entrypoints remain deliberately small:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```

`build.ps1` starts with a cheap source-policy gate that rejects obsolete source roots, frontend-owned native mutation ABI and contract drift before expensive tooling runs.

## Canonical documents

- [`0.2_PROGRESS.md`](0.2_PROGRESS.md) — live status and validation handoff.
- [`docs/0.2/IMPLEMENTATION_ORDER.md`](docs/0.2/IMPLEMENTATION_ORDER.md) — dependency-ordered 0.2 batches.
- [`docs/0.2/FINAL_AUDIT.md`](docs/0.2/FINAL_AUDIT.md) — final source audit and open validation gates.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — system boundaries.
- [`SECURITY.md`](SECURITY.md) and [`docs/security/threat-model.md`](docs/security/threat-model.md) — security guarantees and non-guarantees.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — development rules.
