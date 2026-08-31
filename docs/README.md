# Torca documentation

This directory contains the maintained documentation for the current Torca codebase, the release roadmap and design-reference maquettes. The goal is one canonical place per kind of information, not a growing set of competing status/plan files.

## Current-state documentation

| Document | Canonical responsibility |
| --- | --- |
| [`../README.md`](../README.md) | product overview, supported compositions, repository entry points |
| [`../ARCHITECTURE.md`](../ARCHITECTURE.md) | system ownership, layering, dependency direction and composition |
| [`STATUS.md`](STATUS.md) | current maturity, supported targets/providers and outstanding release evidence |
| [`APP-FLOWS.md`](APP-FLOWS.md) | startup, pairing, messaging, attachments, Radio and lifecycle journeys |
| [`TRANSPORT.md`](TRANSPORT.md) | provider-neutral boundary and current Iroh production behavior |
| [`DEVELOPMENT.md`](DEVELOPMENT.md) | developer setup/workflow and change placement |
| [`TESTING.md`](TESTING.md) | automated/device/soak validation and evidence terminology |
| [`OPERATIONS.md`](OPERATIONS.md) | build/deploy/runtime diagnostics, recovery and incident workflow |
| [`../SECURITY.md`](../SECURITY.md) | security policy, guarantees, limits and reporting guidance |
| [`security/THREAT-MODEL.md`](security/THREAT-MODEL.md) | assets, adversaries, trust boundaries, controls and review triggers |
| [`../PRIVACY.md`](../PRIVACY.md) | local data, network metadata, diagnostics and export/retention behavior |
| [`VERSIONING-AND-RELEASES.md`](VERSIONING-AND-RELEASES.md) | product/build/compatibility versions, tags and release discipline |
| [`../CHANGELOG.md`](../CHANGELOG.md) | notable changes from the documentation baseline forward |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | contribution, architecture, validation and documentation rules |

## Active release roadmap and design reference

[`ROADMAP.md`](ROADMAP.md) is the single maintained release roadmap. The current roadmap targets the `0.3` UX/UI stabilization and polish track: design-system hardening, conversations, navigation/home, pairing/contacts, settings/diagnostics, responsive/accessibility work and final visual regression/release polish.

[`maquettes/README.md`](maquettes/README.md) documents the framework-free interactive UI reference application under `maquettes/view/`. It is the fast design playground for whole flows, responsive hierarchy, themes and deterministic UI states. It may define the **intended visual/inter­action direction** for a `0.3` change, but it is not an implementation of product/domain/network/security behavior and must never override the real Rust/Flutter contracts.

A roadmap item or maquette state describes intended work/design, not current implementation state or validation evidence. Completed behavior must be reflected in the appropriate current-state document; detailed temporary task checklists belong in issues/commits rather than additional roadmap files.

## Architecture decisions

Durable architectural decisions live under [`architecture/decisions/`](architecture/decisions/). ADRs preserve rationale and decision state; they do not replace current-state architecture documentation. An accepted ADR can therefore remain historically useful even after its details are summarized in `ARCHITECTURE.md`.

The current provider decision is [`0006-IROH-PRODUCTION-PROVIDER.md`](architecture/decisions/0006-IROH-PRODUCTION-PROVIDER.md).

## Diagrams

- [`diagrams/architecture.svg`](diagrams/architecture.svg) — current client/runtime/layer/provider ownership.
- [`diagrams/app-flows.svg`](diagrams/app-flows.svg) — startup, pairing and conversation flows.
- [`diagrams/message-delivery.svg`](diagrams/message-delivery.svg) — outbound/inbound ownership and durable feedback.

Diagrams are maintained documentation and must change when their corresponding canonical prose changes materially.

## Source-of-truth order

When information disagrees, use this order:

1. checked-in source, generated contracts, manifests and tests;
2. enforced repository architecture/source policies;
3. current-state architecture/security/privacy/operations documentation;
4. `STATUS.md` and this index;
5. accepted ADRs for rationale/history;
6. `ROADMAP.md` for intended future release work only;
7. `maquettes/` for intended visual/interaction design only; and
8. Git history for retired implementation plans/handoffs.

A roadmap/maquette saying that work or design is intended, or a test file existing, is not evidence that the current commit implements or passed that gate.

## Documentation rules

Document durable current behavior and boundaries:

- responsibility/dependency direction;
- trust/security/privacy consequences;
- user/runtime flows;
- supported targets and provider composition;
- canonical build/deploy/test entry points;
- compatibility/versioning rules; and
- evidence required for claims.

Keep future release direction in the one maintained `ROADMAP.md`. Keep reusable visual/interaction exploration under `maquettes/` rather than creating scattered screenshots or competing prototype directories. Avoid copying volatile inventories such as exact test totals, migration counts, CI job counts, full class lists or timeout constants unless the number itself is a compatibility contract.

Temporary implementation checklists belong in GitHub issues/commits. When the work lands, move durable conclusions into the appropriate canonical page and let Git history preserve the temporary plan.
