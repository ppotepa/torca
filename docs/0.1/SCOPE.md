# Torca 0.1 scope

## Included

### Client identity

- one local installation identity per application data directory;
- generated cryptographic key material;
- local profile name and optional avatar reference;
- safe restart and reload from encrypted local storage.

### Pairing

- create a short-lived invitation;
- join by code and later QR representation;
- explicit approval by both participants;
- authenticated exchange of public identity, onion endpoint and capability material;
- deterministic completion on both clients;
- expiry, cancellation, rejection and retry behavior;
- ephemeral, in-memory relay state only.

### Contacts and conversations

- verified 1:1 contacts;
- block and remove operations with explicit local semantics;
- exactly one initial direct conversation per accepted contact;
- conversation list and conversation detail projections.

### Messaging

- text messages;
- reply references;
- stable message and command identifiers;
- durable local outbox;
- automatic retry with bounded backoff;
- inbound deduplication;
- sent, delivered and read states;
- ordering based on local sequence and protocol timestamps without trusting remote wall clocks as authority.

### Attachments

- architecture and storage contracts for attachments;
- one small encrypted image attachment flow only after text messaging is stable;
- local encrypted cache and explicit size limits.

### Transport

- local Tor process integration;
- per-installation onion service;
- authenticated peer capability handshake;
- direct P2P message delivery through Tor;
- reconnect and resend after temporary loss of connectivity.

### Persistence

- SQLite with SQLCipher-compatible encrypted storage;
- versioned migrations;
- SQL in parameterized files;
- transactional writes for message/outbox and message/receipt pairs;
- no raw database access outside the storage adapter.

### Client platforms

- shared Flutter presentation package;
- Windows host;
- Android host;
- shared Rust engine and typed generated bridge contract;
- lifecycle-safe suspend, resume and background recovery within platform constraints.

### Diagnostics

- structured logs without plaintext message content or secrets;
- explicit engine, storage, Tor, peer and pairing health projections;
- exportable redacted diagnostic bundle.

## Excluded

The following are not 0.1 commitments:

- group conversations;
- calls, voice notes or video;
- multi-device identity synchronization;
- public user discovery;
- phone number or email accounts;
- cloud backup;
- relay-based message delivery or offline mailbox;
- federation;
- desktop platforms other than Windows;
- iOS;
- rich message editing, reactions, stickers or disappearing messages;
- production security certification;
- a stable public SDK.

## Scope control

New work enters 0.1 only when it is required to complete an included flow, remove a security or reliability blocker, or make the architecture testable. Everything else is recorded for a later version without implementation commitment.
