# Architecture documentation

These documents define long-lived engineering rules for Torca.

- [`DOMAIN_MAP.md`](DOMAIN_MAP.md): mini-domain ownership and collaboration.
- [`DEPENDENCY_RULES.md`](DEPENDENCY_RULES.md): allowed dependency direction.
- [`COMMAND_EVENT_MODEL.md`](COMMAND_EVENT_MODEL.md): commands, events and idempotency.
- [`DATA_AND_STORAGE.md`](DATA_AND_STORAGE.md): persistence and SQL rules.
- [`TRANSPORT_AND_PROTOCOL.md`](TRANSPORT_AND_PROTOCOL.md): peer, Tor and wire boundaries.
- [`TORCA_RUNTIME.md`](TORCA_RUNTIME.md): process runtime, actor and workflow coordination.
- [`SECURITY_MODEL.md`](SECURITY_MODEL.md): trust boundaries and secret handling.
- [`TEST_STRATEGY.md`](TEST_STRATEGY.md): testing layers and required seams.
- [`NAMING_AND_LAYOUT.md`](NAMING_AND_LAYOUT.md): crate names and repository structure.

Architecture changes that contradict these rules require an ADR under `docs/decisions`.
