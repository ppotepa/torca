# Torca current source audit

This is a current-state audit, not a declaration that the entire target architecture is complete.

## Verified now

- one process runtime with a bounded mailbox, generic native ABI and explicit shutdown;
- command-only bounded idempotency ledger and state-transition-based snapshot revisions;
- embedded `torca-tor`, with Arti imports confined to that crate;
- optional profile state and idempotent `profile.set` without sentinel names;
- baseline storage schema and deploy-owned data reset;
- shared Windows/Android composition with thin platform adapters;
- root snapshots omit full message history; history is queried in pages;
- cursor-addressed notification projections using the `createdAtMs` wire field;
- relay protocol health checks in runtime and deploy-stack preflight; and
- source policy rejecting obsolete names, external Tor binaries and frontend FFI ownership.

## Known architectural gaps

1. The contract generator does not yet generate complete payload/type models from the JSON schema.
2. Conversation pagination still needs an opaque complete cursor and strict query-error propagation.
3. Some network/delivery adapters can still wait synchronously; those waits must move outside the runtime actor.
4. Flutter still polls bounded root snapshots; a runtime event journal/long-poll API remains planned.
5. Read-receipt policy, capabilities and diagnostics need complete Rust-owned contract projections.
6. Attachment projections need further slimming so root snapshots remain bounded as history grows.

## Validation status

Source and release-artifact checks are distinct from device E2E validation. Keep build metadata, native
ABI, contract and embedded relay endpoint verification mandatory. Run device lifecycle and soak journeys
only when the platform-validation phase is explicitly scheduled.
