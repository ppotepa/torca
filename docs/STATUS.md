# Torca project status

Last reviewed against the repository: **2026-08-14**.

This page is the concise project-status entry point. It summarizes maturity and validation state without duplicating the detailed implementation ledger in [`../0.3_PROGRESS.md`](../0.3_PROGRESS.md).

## Maturity

Torca is **security-sensitive alpha software**. The current source is an actively hardened one-to-one messenger, not a production security claim. There is no independent production security audit, no published stable release in this repository, and the current peer message-key design does not provide Signal-style forward secrecy or post-compromise security.

Use [`../SECURITY.md`](../SECURITY.md) and [`security/threat-model.md`](security/threat-model.md) as the authoritative security documents.

## Current product shape

The active implementation has:

- one responsive Flutter client for Windows and Android;
- one Rust application/runtime shared by both hosts;
- local identities and SQLCipher-backed structured storage;
- explicit pairing through an untrusted ephemeral rendezvous relay;
- direct authenticated peer communication through Tor onion services;
- durable message retry, receipts, searchable/paged history and attachments;
- Safety Number-style contact verification;
- redaction-conscious diagnostics and privacy-aware notification handling;
- experimental mutual-consent Radio Mode with session-specific media keys; and
- an event/deadline-driven runtime baseline intended to avoid periodic application-controlled idle polling.

Groups, calls, multi-device sync, public discovery, cloud backup and a Linux production client are not part of the current supported baseline.

## Runtime and connectivity hardening

The 2026-08-14 runtime hardening pass closed several cross-layer correctness and power issues without changing the project layering or introducing another scheduler:

- Flutter and Android native revision waiters use isolated cancellation identities and no longer confuse actor revision with the durable notification cursor;
- authenticated peer application message kinds are unique, so reactions no longer collide with attachment frames;
- blocking readers on established Tor peer streams wake the process runtime on actual incoming data instead of requiring a periodic peer-stream poll;
- the same real transport event is coalesced into the native presentation snapshot path, allowing an idle Flutter/Android waiter to observe background peer changes without a safety polling timer;
- reconnect backoff is exposed as an exact peer deadline instead of being represented by repeated speculative maintenance;
- blocking or removing one contact disconnects only that relationship rather than shutting down the process-wide peer link;
- Android distinguishes default-route/transport replacement from validation, metering and other capability churn on the same `Network`, avoiding unnecessary destruction of healthy Tor peer streams;
- the exceptional Android native-runtime retry path uses bounded exponential backoff instead of fixed 250 ms retries;
- outgoing attachment maintenance is driven by durable transfer/retry/cancellation state and returns to no application deadline after terminal work; and
- active peer ACK polling uses bounded adaptive backoff rather than a fixed 10 ms loop.

The default remains `AlwaysAvailable`. Arti onion publication/recovery retains its own bounded internal health/recovery observation while a service is published. More aggressive Tor dormancy or a larger Arti lifecycle rewrite remains measurement-gated and is not required for the no-application-idle-polling baseline.

## Validation state

The repository contains automated source, Rust, Flutter/contract and Windows/Android build gates. The detailed engineering ledger records many locally completed checks, but a documentation claim is valid only for the exact command/build/device scenario that was actually executed.

For the runtime-hardening branch, the same checked-out source was locally validated with:

- `cargo fmt --all -- --check`;
- `cargo check --workspace --all-targets --all-features --locked`;
- workspace Clippy with `correctness`, `suspicious` and `perf` promoted to errors; and
- `cargo test --workspace --all-targets --all-features --locked`.

The GitHub Actions workflow could not execute this branch because GitHub rejected job startup for the linked account due a billing/payment lock. No workflow step ran, so that infrastructure failure is **not** evidence that the source failed CI and it must not be reported as a green GitHub Actions run either. Re-run the configured matrix when the account can start Actions jobs again.

The remaining confidence work is primarily platform and end-to-end validation:

- repeated Windows ↔ Android pairing and messaging journeys;
- relay/Tor/network interruption and recovery on real devices;
- attachment resume/cancel/export across reconnects and restarts;
- Radio Mode permission, backgrounding, route-change and recovery soak tests;
- deployment resume/interruption behavior; and
- longer-running battery/runtime traces before enabling more aggressive Tor dormancy policy.

## Security limits that must remain visible

- No independent production security audit has been completed.
- The long-lived pairwise relationship secret is not a Double Ratchet/MLS-style message-key schedule.
- Forward secrecy and post-compromise security are therefore not claimed.
- Tor reduces direct network-location exposure but cannot eliminate timing correlation, censorship or denial of service.
- A compromised endpoint/OS can access plaintext and secrets available to that endpoint.
- The intended recipient can copy, record, export or screenshot content outside Torca's control.
- The pairing relay is deliberately untrusted but can observe the operational metadata required to run rendezvous slots.

## Current engineering sources

Use the documents in this order when they disagree:

1. the checked-in source, generated contracts and tests;
2. [`../ARCHITECTURE.md`](../ARCHITECTURE.md), [`../SECURITY.md`](../SECURITY.md), [`../PRIVACY.md`](../PRIVACY.md) and the threat model for maintained guarantees/boundaries;
3. this status page for a concise maturity summary;
4. [`../0.3_PROGRESS.md`](../0.3_PROGRESS.md) for detailed implementation/validation handoff;
5. focused working records such as [`../BATTERY.MD`](../BATTERY.MD), [`../CONNECTIVITY_HARDENING.md`](../CONNECTIVITY_HARDENING.md) and [`../FINALIZE.md`](../FINALIZE.md).

Planning documents and progress ledgers may contain historical checkpoints. They should never be used to claim that a current binary, platform or release passed a gate that was not actually run.
