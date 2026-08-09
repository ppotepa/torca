# Torca final source audit

The source baseline is complete for the requested unified runtime architecture. Remaining release gates
are execution on real Windows and Android artifacts.

Verified source properties:

- one process runtime with bounded mailbox, immutable revisioned snapshots and explicit shutdown;
- one generic native ABI and one generated Rust/Dart contract;
- one embedded Tor library with Arti imports confined to `torca-tor`;
- optional profile state and idempotent `profile.set` without sentinel names;
- current storage schema with explicit epoch reset owned by deploy tooling;
- shared platform composition with thin Windows/Android adapters;
- root snapshots contain summaries and health, while history is paginated;
- notification events are cursor-addressed and redacted;
- production source policy rejects obsolete names, external Tor binaries and frontend FFI ownership.

Release validation still required:

1. Windows release build and tray lifecycle.
2. Android release APK, foreground service and Activity recreation.
3. Clean-data and warm-cache bootstrap journeys on both platforms.
4. Artifact manifest, native hash, build ID and contract verification after install.
5. Tor retry/stall, relay degraded recovery and controlled shutdown soak tests.
