# Torca documentation

Torca keeps a small set of maintained documents plus a few detailed engineering ledgers. This page explains which document answers which question and prevents release plans, progress notes and evergreen architecture/security documentation from competing as sources of truth.

## Start here

- [`../README.md`](../README.md) — product overview, current capabilities and canonical development entry point.
- [`STATUS.md`](STATUS.md) — concise maturity, validation and remaining-confidence summary.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — contributor workflow, ownership rules and documentation policy.

## Maintained evergreen documents

These describe long-lived behavior and should be updated when the corresponding product boundary changes:

- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — component ownership, dependency direction, client/relay split and runtime model.
- [`../SECURITY.md`](../SECURITY.md) — current security posture, guarantees, limits and reporting guidance.
- [`security/threat-model.md`](security/threat-model.md) — assets, trust boundaries, threats, controls and review triggers.
- [`../PRIVACY.md`](../PRIVACY.md) — local data, network data, notifications/diagnostics and user choices.
- [`../ROADMAP.md`](../ROADMAP.md) — long-lived product and engineering direction.
- [`diagnostics.md`](diagnostics.md) — current diagnostic producers and the Rust deployer collection contract.
- [`../THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md) — principal dependency attribution guidance.

When an evergreen document disagrees with the current source, treat the source and enforced/generated contracts as authoritative and fix the documentation.

## Current engineering handoff

- [`../0.3.md`](../0.3.md) — current architecture-track plan and invariants.
- [`../0.3_PROGRESS.md`](../0.3_PROGRESS.md) — detailed implementation and validation ledger for the current track.

The `0.3` label is a planning/work-track label. The package version in `Cargo.toml` is separate build metadata; neither label by itself establishes release maturity.

## Focused working records

These are useful design/acceptance records, but they are not evergreen product specifications:

- [`../BATTERY.MD`](../BATTERY.MD) — attention/demand/evidence/deadline runtime design and battery-hardening status.
- [`../CONNECTIVITY_HARDENING.md`](../CONNECTIVITY_HARDENING.md) — connectivity supervision invariants and implementation notes.
- [`../FINALIZE.md`](../FINALIZE.md) — detailed 0.3 implementation/release-gate checklist.
- [`FINALIZE_MANUAL_RUNBOOK.md`](FINALIZE_MANUAL_RUNBOOK.md) — real-device manual acceptance procedure.

These files may contain dated checkpoints or historical implementation detail. Once a principle becomes stable, move the durable conclusion into the appropriate evergreen document instead of treating the working record as a permanent specification.

## Navigation READMEs

Short README files under `apps`, `crates`, `services`, `tests` and similar top-level directories are navigation aids. They should explain ownership and point back to the maintained architecture rather than re-document individual APIs or duplicate build instructions.

## Documentation rules

Prefer documenting stable facts such as:

- what Torca is and is not trying to be;
- which layer owns a responsibility;
- which trust boundary is crossed;
- why the client/relay split exists;
- which workflow is canonical for contributors;
- security and privacy guarantees/non-guarantees; and
- what evidence is required before a validation claim is made.

Avoid maintaining prose copies of rapidly changing implementation details such as exact schema/contract versions, migration counts, timeout constants, generated DTO field lists, complete crate inventories, temporary refactor names or test counts. Those are better represented by source, generated schemas, tests, CI configuration and Git history.

## Status and validation language

Use precise evidence language:

- **implemented** means the source path is present and composed;
- **source-validated** means named static/build/test gates were actually run;
- **platform-built** means the named platform artifact was built;
- **device-validated** means the named scenario was exercised on real devices; and
- **audited** should only be used for an actual independent security review.

Do not turn a checked box in a planning document into a stronger claim than the evidence supports.