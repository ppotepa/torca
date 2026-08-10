# Torca implementation order

This repository implements one Windows/Android baseline: one Flutter presentation, one process Rust
runtime, one canonical operation contract, and thin operating-system adapters.

## Completed foundations

1. Source policy, storage baseline and a single process runtime are in place.
2. Windows and Android share `PlatformServices`, `torca-tor`, a generic native ABI and one Flutter worker.
3. Bootstrap, profile creation, relay probing, lifecycle forwarding, bounded command idempotency and
   state-transition-based snapshot revisions are implemented.
4. Deploy tooling embeds the relay endpoint and verifies relay protocol health before installation.

## Remaining hardening work

1. Make the contract schema generate complete Rust and Dart payload/type models instead of validating an
   operation allow-list plus a checked-in Dart template.
2. Correct the conversation-page cursor contract and propagate query failures instead of decoding them as
   empty successful pages.
3. Move blocking peer/Tor delivery waits out of the runtime actor and return typed completion events.
4. Replace frequent root-snapshot polling with a cursor-addressed runtime event journal or long-poll API.
5. Move all protocol settings/capabilities, including read-receipt policy, to Rust and expose real
   diagnostics projections.
6. Keep root snapshots bounded by removing legacy message/attachment projections and using targeted
   queries.
7. Validate artifacts and lifecycle behavior on Windows and Android when platform testing is scheduled.

Source checks, artifact validation and device E2E validation are separate gates. Physical-device E2E is
not a required local development gate unless explicitly scheduled.
