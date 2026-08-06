# Native engine method channel

Flutter uses one versioned platform channel for Windows and Android:

```text
torca.engine.v1
```

The default application build uses `MethodChannelEngineGateway`. The in-memory engine is enabled only with:

```text
--dart-define=TORCA_USE_MEMORY_GATEWAY=true
```

A release build must not set that flag.

## Flutter to native methods

### `snapshot`

Arguments:

```text
{ contractVersion: 1 }
```

Returns an application snapshot map.

### `execute`

Arguments:

```text
{
  contractVersion: 1,
  command: { type: ..., command fields... }
}
```

Supported command types:

- `createIdentity` — `identityIdHex`, `displayName`, `atMs`;
- `startPairing` — `sessionIdHex`, `code`, `expiresAtMs`;
- `queueMessage` — `messageIdHex`, `conversationIdHex`, `body`, `atMs`.

Returns:

```text
{ ok: bool, kind: string, error: string? }
```

## Native to Flutter callback

### `snapshotChanged`

The native host invokes this method with the complete current snapshot. Partial patches are prohibited in 0.1.

## Snapshot map

```text
{
  contractVersion: 1,
  identity: { displayName: string }?,
  contacts: [
    { id: string, onionAddress: string, status: string }
  ],
  conversations: [
    { id: string, contactId: string, status: string }
  ],
  messages: [
    {
      id: string,
      conversationId: string,
      body: string,
      direction: string,
      status: string
    }
  ]
}
```

All identifiers are canonical 32-character lowercase hexadecimal values. Times are bounded Unix milliseconds. Unknown contract versions must be rejected rather than guessed.

## Host requirements

Each native runner must:

1. create exactly one Rust engine owner;
2. register the method channel before Flutter submits commands;
3. serialize all engine mutations through `ClientEngine`;
4. translate primitive channel maps to the generated bridge contract;
5. push a full snapshot after successful state-changing commands;
6. keep workflow state out of Kotlin, C++ and Dart;
7. return redacted errors only;
8. unregister callbacks during final engine shutdown.

GATE-003 closes only after both platform runners execute this contract against the real Rust engine library and the memory gateway is excluded from release artifacts.
