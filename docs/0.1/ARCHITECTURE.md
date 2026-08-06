# Torca 0.1 architecture

This document fixes the concrete component set planned for 0.1. Long-lived dependency rules remain in `docs/architecture`.

## Client composition

```text
apps/client/flutter
    |
    v
crates/platform/torca-bridge
    |
    v
crates/application/torca-client-engine
    |                |
    |                +--> torca-projections
    |
    +--> domains/
    |      torca-identity
    |      torca-contacts
    |      torca-pairing
    |      torca-conversations
    |      torca-messaging
    |      torca-receipts
    |      torca-attachments
    |      torca-presence
    |      torca-notifications
    |
    +--> infrastructure adapters selected by the app
           torca-storage-sqlite
           torca-crypto
           torca-peer
           torca-transport-tor
           torca-rendezvous-client
           torca-file-storage
```

Protocol crates are used only at external boundaries:

```text
torca-wire
  +-- torca-peer-protocol
  +-- torca-pairing-protocol
  +-- torca-relay-protocol
```

## Relay composition

```text
services/relay
    |
    +--> torca-relay-protocol
    +--> bounded in-memory slot registry
    +--> connection and expiry workers
```

The relay does not link client domain crates.

## Required engine workers

The ClientEngine coordinates bounded workers that report typed events back to the actor:

- pairing rendezvous session worker;
- inbound peer listener;
- outbound delivery worker;
- receipt delivery worker;
- retry scheduler;
- Tor lifecycle worker;
- attachment preparation and transfer worker;
- projection publisher;
- diagnostic health collector.

Workers cannot mutate domain or database state directly. They submit completion or observation events to the engine.

## Required durable records

The 0.1 storage model must represent at least:

- installation identity and local profile;
- processed command identifiers;
- pairing sessions and resumable local state;
- contacts and authorized peer endpoints;
- direct conversations and read boundaries;
- messages and reply references;
- receipts;
- outbound outbox items and attempts;
- inbound envelope deduplication;
- attachment metadata and encrypted blob references;
- schema version and migration history.

## Pairing boundary

```text
Pairing domain
    -> Rendezvous port
        -> torca-rendezvous-client
            -> torca-relay-protocol
                -> services/relay

PairingCompleted domain event
    -> application handler
        -> create contact
        -> create direct conversation
        -> register peer endpoint and capability
        -> publish updated projections
```

## Messaging boundary

```text
SendMessage command
    -> messaging validates and creates stable MessageId
    -> storage transaction writes message + outbox + command result
    -> engine schedules delivery
    -> peer protocol maps domain payload to wire DTO
    -> crypto seals payload
    -> peer session sends through Tor
    -> protocol acknowledgement commits outbox progress
    -> receipt later updates user-visible delivery state
```

## Startup order

1. initialize redacted diagnostics;
2. open platform key provider;
3. open and migrate encrypted storage;
4. load or request creation of installation identity;
5. start ClientEngine actor;
6. recover durable work and incomplete local workflows;
7. start Tor and peer listener;
8. start bridge snapshot publication;
9. report ready or degraded state.

## Shutdown order

1. stop accepting new external commands;
2. cancel active pairing and network operations safely;
3. stop inbound listener and delivery workers;
4. flush required durable engine state;
5. stop Tor;
6. close storage;
7. publish final stopped state to the host.

## Deliberate 0.1 simplifications

- one identity per application data directory;
- direct 1:1 conversations only;
- one active engine process per data directory;
- in-memory relay only;
- text first, one bounded image flow later;
- no generic plugin framework;
- no distributed event bus;
- no separate microservices for domains.
