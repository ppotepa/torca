# torca-client-engine

## Purpose

Provide the application-level serialized workflow engine used by Rust runtime composition. The public
process boundary is `TorcaRuntime` in `torca-native`; Flutter and platform hosts do not call this crate
directly.

## Owns

- command dispatch and workflow coordination;
- application event handling;
- durable-job scheduling and retry orchestration; and
- peer, timer and lifecycle completion handling.

## Does not own

Native handle lifetime, ABI serialization, platform services, raw SQL, cryptographic algorithms,
protocol byte encoding, Flutter state or system notification rendering.

The process runtime owns the externally visible revision, request idempotency and controlled shutdown.
Network work must not hold the runtime actor while waiting for an unbounded peer or Tor operation.
