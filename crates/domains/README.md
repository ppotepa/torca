# Domain libraries

Domain crates define Torca's product concepts and invariants independently from persistence, networking and UI.

The active domains cover identity, contacts, conversations, pairing, messaging, receipts, attachments, presence and notification intent. Each domain should own its valid states/transitions and the ports required by those semantics.

Domains must not depend on SQLCipher, Arti, sockets, Flutter, native ABI or OS APIs. This boundary is enforced by the repository architecture policy.

Presence is derived from observed peer/activity facts rather than a central presence service. Notification domain code decides privacy-safe notification intent; platform code performs OS delivery.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) for cross-layer flows. Individual domain APIs are documented in their Rust source rather than maintained as parallel per-crate READMEs.