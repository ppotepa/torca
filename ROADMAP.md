# Torca roadmap

The active engineering target is Torca **0.1** validation, followed by planning for **0.2**.

## Core implementation state

All 20 dependency-ordered source batches in [`docs/0.1/IMPLEMENTATION_ORDER.md`](docs/0.1/IMPLEMENTATION_ORDER.md) have committed code and documentation.

This means the original **core source-roadmap coverage is 100%**. It does not mean release validation is complete. The remaining production integrations and owner-run checks are listed in:

- [`0.1_PROGRESS.md`](0.1_PROGRESS.md);
- [`docs/0.1/KNOWN_LIMITATIONS.md`](docs/0.1/KNOWN_LIMITATIONS.md);
- [`docs/0.1/RELEASE_CHECKLIST.md`](docs/0.1/RELEASE_CHECKLIST.md);
- [`docs/0.1/TEST_MATRIX.md`](docs/0.1/TEST_MATRIX.md).

## 0.1 UI productization track

The shared-client product/UI source pass defined in [`docs/0.1/UI_IMPLEMENTATION_ORDER.md`](docs/0.1/UI_IMPLEMENTATION_ORDER.md) is source-complete through UI-15. The final source audit is recorded in [`docs/0.1/UI_FINAL_AUDIT.md`](docs/0.1/UI_FINAL_AUDIT.md).

The UI track did not reopen the architecture: Flutter remains one responsive presentation client and Rust/runtime remains the owner of messaging, pairing, Tor, peer state, persistence, cryptography and background workflows.

Platform and end-to-end validation remain release gates and are always recorded in [`0.1_PROGRESS.md`](0.1_PROGRESS.md).

## 0.2 planning rule

0.2 should be planned as a concrete improvement over the validated 0.1 baseline. Source candidates may be collected before validation is complete, but no 0.2 item should silently redefine an unresolved 0.1 release gate.
