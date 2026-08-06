# Torca 0.1 risk register

Risks are reviewed when their owning milestone begins and when their status changes.

## R1 — Excessive crate fragmentation

Impact: high maintenance cost, unclear contracts and slow iteration.

Mitigation: create a crate only for a mini-domain or capability with independent vocabulary, invariants and tests. Keep small concepts as private modules. Review the dependency graph during M1.

## R2 — ClientEngine becomes a new monolith

Impact: domain rules and infrastructure details accumulate in the actor.

Mitigation: keep the actor as coordinator, require domain APIs for transitions, execute long I/O in workers and review engine modules against `CLIENT_ENGINE.md` in every milestone.

## R3 — Message loss at crash boundaries

Impact: accepted user messages disappear or appear sent incorrectly.

Mitigation: transactional message/outbox writes, stable identifiers, explicit acknowledgement levels and failure-injection tests in M4 and M5.

## R4 — Duplicate or divergent pairing completion

Impact: one client creates a contact while the other remains pending, or duplicate contacts appear.

Mitigation: transcript-bound completion, command idempotency, unique remote identity constraints and two-engine restart tests in M3.

## R5 — Tor lifecycle differs by platform

Impact: desktop works while Android loses listeners or delivery during background transitions.

Mitigation: platform-neutral Tor and engine state model, explicit host lifecycle events, Android background constraints tested in M6 and degraded-state projections.

## R6 — Bridge contract becomes handwritten twice

Impact: Rust and Dart models drift and UI implements compensating logic.

Mitigation: one source contract, deterministic generation, committed compatibility tests and CI verification in M1 and M6.

## R7 — SQL spreads outside storage

Impact: migration complexity, security review gaps and broken transaction ownership.

Mitigation: ADR 0003, dependency checks, repository review rule and centralized SQL roots established before M2 expands schema.

## R8 — Cryptographic format churn

Impact: incompatible stored identities or messages and unrecoverable test installations.

Mitigation: explicit format versions and test vectors from first implementation; migration commitment begins with first distributable test build.

## R9 — Diagnostics leak sensitive data

Impact: privacy compromise through logs or exported bundles.

Mitigation: secret classification, redacted identifier types, structured fields, allowlisted export schema and security tests in M2 and M7.

## R10 — Old implementation is copied with hidden coupling

Impact: legacy architecture returns under new crate names.

Mitigation: import only one reviewed capability at a time, write target contracts and tests first, and treat old code as reference rather than migration source.

## R11 — Main-only development causes unstable commits

Impact: `main` becomes temporarily unusable.

Mitigation: small dependency-ordered batches, unused isolated modules for incomplete code, one validation entrypoint and mandatory status updates. Revisit ADR 0004 when contributor concurrency increases.

## R12 — Scope expands before the core journey works

Impact: delayed 0.1 and shallow implementations across many features.

Mitigation: `SCOPE.md` is authoritative, attachments start after text reliability, and out-of-scope requests are deferred unless they remove a core blocker.
