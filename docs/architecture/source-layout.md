# Source layout and documentation

Torca keeps public crate roots small and groups state machines by ownership rather than by generic technical layers.

## Root module documentation

A real Rust module root (`lib.rs`, `mod.rs`, or a file declared with `mod name;`) should use a short `//!` header describing:

- what state the module owns;
- what boundary it exposes;
- what it deliberately does not own.

Do not document obvious field accessors or repeat type names in prose.

## `include!` implementation fragments

Large actors are physically split with `include!` while preserving one Rust namespace. Included fragments are not independent modules, so they must **not** use inner module docs (`//!`) when the expansion occurs after other items or inside an `impl` block.

Use a normal source header instead:

```rust
// Responsibility: bounded engine mailbox and actor lifetime.
```

The filename should describe the responsibility (`mailbox.rs`, `command_dispatch.rs`, `persistence.rs`) rather than an implementation phase (`part2.rs`).

## Comments

Keep comments that explain invariants or ownership, for example:

- ACK is emitted only after durable ingress;
- a Tor recovery result is valid only for the current recovery epoch;
- a cache is presentation-only and cannot establish durability;
- a protected-state buffer must be wiped after decode/store.

Delete comments that only narrate the syntax immediately below them or preserve obsolete implementation history.

## Current ownership map

- `torca-runtime/actor`: process application scheduler, leases, diagnostics and runtime commands;
- `torca-client-engine/engine`: single-writer domain engine plus mailbox;
- `torca-pairing-coordinator/runtime`: invitation/approval/completion and protected restart state;
- `torca-pairing-coordinator/core`: rendezvous transport state and crypto boundary;
- `torca-peer-link/owner`: authenticated peer sessions, ACK/reconnect and transport telemetry;
- `torca-native/native_runtime`: platform host projections, commands and startup lifecycle;
- `torca-native/torca_runtime`: process registry, actor request router and C/JNI ABI.

The compile-safety validation pass checks that included fragments use a form of documentation that is legal at their expansion site.
