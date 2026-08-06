# torca-conversations

## Purpose

Own the container and user-level state of a conversation independently from message delivery.

## Owns

- `ConversationId`;
- conversation type, initially `Direct`;
- member identity references;
- active and archived state;
- unread boundary and last-read marker semantics;
- conversation metadata required by projections;
- repository port.

## Does not own

Message bodies, outbox attempts, peer transport, presence or Flutter navigation state.

## Planned commands

`CreateDirectConversation`, `ArchiveConversation`, `RestoreConversation`, `MarkConversationRead`.

## 0.1 completion

Pairing creates exactly one direct conversation for a verified contact, and read-boundary updates are monotonic and idempotent.
