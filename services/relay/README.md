# Torca rendezvous relay

The relay temporarily connects two active clients participating in contact pairing and forwards bounded opaque pairing frames between them.

It owns ephemeral slot lifecycle, authorization/capability checks, expiry/resource bounds, protocol errors and operational service behavior. It is deliberately disposable: restarting it may invalidate active pairing attempts but must not affect established conversation history or normal peer messaging.

The relay is **not**:

- an account/identity provider;
- a public contact directory;
- a normal message relay;
- an offline mailbox;
- a conversation/history store;
- a central presence service;
- the source of truth for pairing completion.

Clients treat the relay as untrusted for confidentiality and identity. Pairing content and relationship approval remain client-side cryptographic/application responsibilities.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) and [`../../docs/security/threat-model.md`](../../docs/security/threat-model.md).