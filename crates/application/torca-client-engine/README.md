# torca-client-engine

## Purpose

Provide one serialized client-side command and event loop that coordinates all state-changing workflows.

## Owns

- bounded mailbox and engine lifecycle;
- command dispatch and idempotency coordination;
- application event handlers;
- durable-job scheduling and retry orchestration;
- peer, timer and platform completion events;
- snapshot revision publication;
- startup recovery and ordered shutdown.

## Does not own

Domain invariants, raw SQL, cryptographic algorithms, protocol byte encoding, Flutter state or OS notification calls.

## Planned API

```text
ClientEngine::start(config, adapters)
ClientEngineHandle::submit(command)
ClientEngineHandle::subscribe_snapshots()
ClientEngineHandle::health()
ClientEngineHandle::shutdown()
```

## 0.1 completion

One engine instance coordinates identity, pairing, contacts, conversations, messaging and delivery recovery without blocking its actor loop on unbounded I/O.
