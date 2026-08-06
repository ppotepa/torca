# Torca 0.1

Torca 0.1 is the first clean engineering baseline for the new repository. It is not a production release and does not claim complete anonymity or feature parity with mature messengers.

## Start here

Read [`../../0.1_PROGRESS.md`](../../0.1_PROGRESS.md) before changing the repository. It is the only live implementation and handoff record.

## Goal

Deliver a coherent private 1:1 messenger skeleton in which two client installations can create local identities, pair through an ephemeral relay, establish a contact, exchange encrypted text messages directly through Tor, persist state locally, recover from interruption, and render the same application state on Windows and Android.

## Planning documents

- [`SCOPE.md`](SCOPE.md) — product boundary.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — exact 0.1 component composition and flows.
- [`ROADMAP.md`](ROADMAP.md) — milestones and exit criteria.
- [`IMPLEMENTATION_ORDER.md`](IMPLEMENTATION_ORDER.md) — concrete dependency-aware sequence.
- [`TRACEABILITY.md`](TRACEABILITY.md) — mapping from required capabilities to domains and milestones.
- [`RISKS.md`](RISKS.md) — active technical and product risks.
- [`TOOLCHAIN.md`](TOOLCHAIN.md) — pinned development baseline.
- [`FOUNDATION_CONTRACTS.md`](FOUNDATION_CONTRACTS.md) — identifiers, time, command/event, error and cancellation contracts.
- [`DEFINITION_OF_DONE.md`](DEFINITION_OF_DONE.md) — release completion rules.

`STATUS.md` is only a compatibility pointer. Current progress is maintained at the repository root.

## Validation ownership

Implementation agents record static review and add tests with each batch. The project owner executes the full local validation suite and supplies results for addition to `0.1_PROGRESS.md`. Pending local test evidence is recorded explicitly and is never represented as a passing result.

## Release posture

- APIs and storage formats may change within 0.1 development.
- Security-sensitive formats must still be explicitly versioned.
- Migration support is required after the first distributable 0.1 test build.
- No feature is considered complete only because its UI exists.
- The old repository may provide test vectors or implementation references, but code is imported only after boundary review.
