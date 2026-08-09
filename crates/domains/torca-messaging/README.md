# torca-messaging

## Purpose

Own text-message semantics and the valid lifecycle of inbound and outbound messages.

## Owns

- `MessageId`, direction and conversation reference;
- text body validation and bounded size;
- reply references;
- draft, queued, sending, sent, delivered, read, failed and cancelled states;
- valid state transitions and retry eligibility;
- stable sender and recipient facts;
- message repository and durable-enqueue ports.

## Does not own

Socket connections, embedded Tor state, SQL tables, encryption algorithm details, Flutter message bubbles or operating-system notifications.

## Planned commands

`ComposeMessage`, `QueueMessage`, `AcceptIncomingMessage`, `RetryMessage`, `CancelMessage`.

## Planned events

`MessageComposed`, `MessageQueued`, `IncomingMessageAccepted`, `MessageSendRequested`, `MessageSent`, `MessageFailed`, `MessageCancelled`.

## State rule

State transitions are explicit methods, not arbitrary field updates. Remote timestamps are metadata and cannot force an invalid local transition.

## 0.1 completion

Text messages and replies pass pure invariant tests, retries reuse stable identifiers, and duplicate incoming messages return the original acceptance outcome.
