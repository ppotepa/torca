# Domain map

Torca divides product behavior into focused mini-domains. Each mini-domain is a Rust library with a small public API.

## Identity

Owns installation identity, public identity, local profile and identity lifecycle. It declares ports for key material and identity persistence. It does not own contacts, pairing transport or peer sessions.

## Contacts

Owns verified relationship state, trust state, block/remove semantics and contact metadata. It does not own live network connectivity or message history.

## Pairing

Owns invitation and pairing-session state machines, approval, rejection, cancellation, expiry and the verified pairing result. It uses rendezvous and cryptographic ports but does not create database rows directly.

## Conversations

Owns direct conversation identity, membership, archive state, unread boundary and conversation-level metadata. It does not own message delivery.

## Messaging

Owns message creation, content metadata, reply references, direction, send lifecycle and valid message state transitions. It requests durable enqueueing through ports but does not send sockets.

## Receipts

Owns delivered and read receipt semantics, monotonicity, idempotency and mapping to message state. Transport acknowledgements are not user-level receipts.

## Attachments

Owns attachment metadata, limits, lifecycle and references from messages. Encryption, blob storage and transfer are ports implemented elsewhere.

## Presence

Owns the derived application concept of contact availability. It combines peer connectivity, endpoint capability, last activity and conversation focus without collapsing them into one boolean.

## Notifications

Owns notification intent and privacy-safe display policy. Operating-system notification APIs are platform adapters.

## Application collaborations

Cross-domain effects are explicit application workflows. Examples:

```text
PairingCompleted
    -> create Contact
    -> create direct Conversation
    -> register peer endpoint

IncomingMessageAccepted
    -> persist message
    -> schedule delivery receipt
    -> update conversation projection
    -> create notification intent
```

A domain must not call another domain's repository or infrastructure adapter directly. Shared concepts should be expressed through stable identifiers or narrow contracts, not shared mutable aggregates.
