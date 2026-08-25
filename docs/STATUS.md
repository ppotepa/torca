# Torca project status

Last reviewed against the repository: **2026-08-21**.

This page is the concise project-status entry point. It summarizes maturity and validation state; checked-in source and executed evidence remain authoritative.

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
- an event/deadline-driven runtime baseline with a one-shot background grace and no default periodic rendezvous wake.

Groups, calls, multi-device sync, public discovery, cloud backup and a Linux production client are not part of the current supported baseline.

## Validation state

The repository contains automated source, Rust, Flutter/contract and Windows/Android build gates. The detailed engineering ledger records many locally completed checks, but a documentation claim is valid only for the exact command/build/device scenario that was actually executed.

The remaining confidence work is primarily platform and end-to-end validation:

- repeated Windows ↔ Android pairing and messaging journeys;
- relay/Tor/network interruption and recovery on real devices;
- attachment resume/cancel/export across reconnects and restarts;
- Radio Mode permission, backgrounding, route-change and recovery soak tests;
- deployment resume/interruption behavior; and
- longer-running battery/runtime traces before enabling more aggressive communication-provider dormancy policy.

The GitHub Actions workflow is configured to run the automated matrix, but the existence of the workflow is not proof that the current commit is green. Check the actual Actions result before citing CI as evidence.

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
4. [`architecture/runtime-control.md`](architecture/runtime-control.md) for runtime/power invariants;
5. [`validation/runtime-power.md`](validation/runtime-power.md) and [`diagnostics.md`](diagnostics.md) for evidence and collection;
6. historical working records only as historical context.

Planning documents and progress ledgers may contain historical checkpoints. They should never be used to claim that a current binary, platform or release passed a gate that was not actually run.
