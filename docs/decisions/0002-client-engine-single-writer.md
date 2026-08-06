# ADR 0002: ClientEngine as the single writer

- Status: accepted
- Date: 2026-08-06

## Context

Messaging, pairing, retries, platform lifecycle and peer events can mutate related state concurrently. Multiple independent state machines previously created divergence between UI and runtime behavior.

## Decision

All client state-changing operations pass through one ClientEngine actor per application data directory. The actor serializes commands and typed completion events. Long-running I/O executes outside the actor and returns results through its bounded mailbox.

## Consequences

- local mutation order is deterministic;
- UI cannot bypass domain workflows;
- shutdown and recovery have one owner;
- mailbox design and backpressure become explicit concerns;
- the actor must remain a coordinator rather than absorbing domain rules or blocking I/O.
