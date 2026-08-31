# Torca documentation

This directory contains the maintained documentation for the current Torca codebase plus clearly separated roadmap and dated evidence. The goal is one canonical place per kind of information, not a growing set of competing status/plan files.

## Current-state documentation

| Document | Canonical responsibility |
| --- | --- |
| [`../README.md`](../README.md) | product overview, supported compositions, repository entry points |
| [`../ARCHITECTURE.md`](../ARCHITECTURE.md) | system ownership, layering, dependency direction and composition |
| [`STATUS.md`](STATUS.md) | current maturity, supported targets/providers and outstanding release evidence |
| [`app-flows.md`](app-flows.md) | startup, pairing, messaging, attachments, Radio and lifecycle journeys |
| [`transport.md`](transport.md) | provider-neutral boundary and current Iroh production behavior |
| [`development.md`](development.md) | developer setup/workflow and change placement |
| [`testing.md`](testing.md) | automated/device/soak validation and evidence terminology |
| [`operations.md`](operations.md) | build/deploy/runtime diagnostics, recovery and incident workflow |
| [`../SECURITY.md`](../SECURITY.md) | security policy, guarantees, limits and reporting guidance |
| [`security/threat-model.md`](security/threat-model.md) | assets, adversaries, trust boundaries, controls and review triggers |
| [`../PRIVACY.md`](../PRIVACY.md) | local data, network metadata, diagnostics and export/retention behavior |
| [`versioning-and-releases.md`](versioning-and-releases.md) | product/build/compatibility versions, tags and release discipline |
| [`../CHANGELOG.md`](../CHANGELOG.md) | notable changes from the documentation baseline forward |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | contribution, architecture, validation and documentation rules |

## Active release roadmap

[`ROADMAP.md`](ROADMAP.md) is the single maintained release roadmap. The current roadmap targets the `0.3` UX/UI stabilization and polish track: design-system hardening, conversations, navigation/home, pairing/contacts, settings/diagnostics, responsive/accessibility work and final visual regression/release polish.

A roadmap item describes intended work, not current implementation state or validation evidence. Completed behavior must be reflected in the appropriate current-state document; detailed temporary task checklists belong in issues/commits rather than additional roadmap files.

## Architecture decisions

Durable architectural decisions live under [`architecture/decisions/`](architecture/decisions/). ADRs preserve rationale and decision state; they do not replace current-state architecture documentation. An accepted ADR can therefore remain historically useful even after its details are summarized in `ARCHITECTURE.md`.

The current provider decision is [`0006-iroh-production-provider.md`](architecture/decisions/0006-iroh-production-provider.md).

## Diagrams

- [`diagrams/architecture.svg`](diagrams/architecture.svg) — current client/runtime/layer/provider ownership.
- [`diagrams/app-flows.svg`](diagrams/app-flows.svg) — startup, pairing and conversation flows.
- [`diagrams/message-delivery.svg`](diagrams/message-delivery.svg) — outbound/inbound ownership and durable feedback.

Diagrams are maintained documentation and must change when their corresponding canonical prose changes materially.

## Validation evidence

[`validation/`](validation/) contains dated audit/soak/measurement reports. These are evidence snapshots for the commit, device, environment and procedure recorded in each report. They are intentionally not rewritten to look current.

Read [`validation/README.md`](validation/README.md) before using a dated report in a claim.

## Source-of-truth order

When information disagrees, use this order:

1. checked-in source, generated contracts, manifests and tests;
2. enforced repository architecture/source policies;
3. current-state architecture/security/privacy/operations documentation;
4. `STATUS.md` and this index;
5. accepted ADRs for rationale/history;
6. `ROADMAP.md` for intended future release work only;
7. dated validation reports for evidence about the run they describe;
8. Git history for retired implementation plans/handoffs.

A roadmap saying that work is intended, or a test file existing, is not evidence that the current commit passed a gate.

## Documentation rules

Document durable current behavior and boundaries:

- responsibility/dependency direction;
- trust/security/privacy consequences;
- user/runtime flows;
- supported targets and provider composition;
- canonical build/deploy/test entry points;
- compatibility/versioning rules; and
- evidence required for claims.

Keep future release direction in the one maintained `ROADMAP.md`. Avoid copying volatile inventories such as exact test totals, migration counts, CI job counts, full class lists or timeout constants unless the number itself is a compatibility contract.

Temporary implementation checklists belong in GitHub issues/commits. When the work lands, move durable conclusions into the appropriate canonical page and let Git history preserve the temporary plan.
