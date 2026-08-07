# Torca roadmap

The active engineering target is Torca **0.1**.

## Core implementation state

All 20 dependency-ordered source batches in [`docs/0.1/IMPLEMENTATION_ORDER.md`](docs/0.1/IMPLEMENTATION_ORDER.md) have committed code and documentation.

This means the original **core source-roadmap coverage is 100%**. It does not mean release validation is complete. The remaining production integrations and owner-run checks are listed in:

- [`0.1_PROGRESS.md`](0.1_PROGRESS.md);
- [`docs/0.1/KNOWN_LIMITATIONS.md`](docs/0.1/KNOWN_LIMITATIONS.md);
- [`docs/0.1/RELEASE_CHECKLIST.md`](docs/0.1/RELEASE_CHECKLIST.md);
- [`docs/0.1/TEST_MATRIX.md`](docs/0.1/TEST_MATRIX.md).

## 0.1 UI productization track

A focused shared-client product/UI pass is active after core source completion. Its dependency order and design constraints are defined in [`docs/0.1/UI_IMPLEMENTATION_ORDER.md`](docs/0.1/UI_IMPLEMENTATION_ORDER.md).

The UI track does not reopen the architecture: Flutter remains one responsive presentation client and Rust/runtime remains the owner of messaging, pairing, Tor, peer state, persistence, cryptography and background workflows.

The exact active UI batch and validation state are always recorded in [`0.1_PROGRESS.md`](0.1_PROGRESS.md).

## Planning rule

No 1.0 scope is planned until the 0.1 release checklist is completed and its limitations are understood from real Windows, Android and Tor tests.
