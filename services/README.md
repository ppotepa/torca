# Services

Torca currently contains one server-side component: the [`relay`](relay) rendezvous service used during contact pairing.

The client remains the owner of user identity, conversation history and normal message delivery. Services must not gradually become an implicit central account, presence, mailbox or message-routing layer without an explicit product/architecture decision.

See the root [`ARCHITECTURE.md`](../ARCHITECTURE.md) for the client/relay boundary.