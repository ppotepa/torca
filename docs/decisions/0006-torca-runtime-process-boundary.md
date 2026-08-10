# ADR 0006: TorcaRuntime is the process boundary

- Status: accepted
- Date: 2026-08-10
- Supersedes: ADR 0002 for the external runtime boundary

## Context

ADR 0002 established a single-writer `ClientEngine` for application workflow coordination. The unified
Windows/Android client now needs one process-owned Rust runtime with native handles, lifecycle,
notification cursors, bootstrap state and a generic ABI. Calling the application engine itself the public
runtime boundary obscures those responsibilities.

## Decision

`TorcaRuntime` is the process and native-ABI boundary. It owns runtime lifetime, external request
serialization, command idempotency, observable snapshot revision, lifecycle and controlled shutdown.
`torca-client-engine` remains an internal application coordination component where composition uses it;
it is not exposed to Flutter or platform hosts.

Long-running network work is scheduled outside the single-writer state transition path and returns typed
completion results. This is a required invariant; remaining synchronous adapters are tracked as
hardening work rather than an alternative architecture.

## Consequences

- one stable runtime identity survives presentation-worker reattachment;
- ABI consumers have one owner for timeouts and shutdown;
- application coordination remains modular instead of becoming a runtime god object; and
- documentation uses `TorcaRuntime` for process-level behavior and `ClientEngine` only for the internal
  application crate.
