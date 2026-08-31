# Domain libraries

Domain crates define Torca product concepts and invariants independently from persistence, concrete communication providers and UI.

Active domains cover identity, contacts, conversations, pairing, messaging, receipts, attachments, presence, notifications and Radio. Each domain owns valid states/transitions and product meaning; application/infrastructure layers coordinate and execute them.

Domains must not depend on SQLCipher repositories, Iroh/QUIC/provider SDK types, Flutter, native ABI or OS APIs. This boundary is enforced by repository architecture policy.

Presence is derived from observed peer/activity facts rather than a central presence service. Notification domain code decides privacy-safe notification intent; platform code performs OS delivery.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md). Individual APIs are documented in Rust source rather than parallel per-crate READMEs.
