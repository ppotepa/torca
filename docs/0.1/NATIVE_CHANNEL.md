# Native client runtime ABI

Torca uses one native runtime boundary for every Flutter target. The same Dart `FfiEngineGateway` loads the same Rust `torca-native` ABI on Windows and Android.

Platform-specific Kotlin/C++ must not implement a second command router or workflow state machine.

## Libraries

```text
Windows: torca_bridge.dll
Android: libtorca_bridge.so
```

The library owns one `ClientEngineActor` and one `EngineBridge` per application process.

## Contract version

`torca_contract_version()` returns the same version as the generated Flutter contract. A mismatch is a startup error; versions are never guessed.

## Lifetime

```text
torca_engine_new
    -> command / snapshot calls
    -> torca_engine_close
    -> torca_engine_destroy
```

The Flutter application owns the native handle and destroys it exactly once.

## Commands

The ABI exposes narrow command functions rather than generic JSON command dispatch:

- `torca_engine_create_identity`
- `torca_engine_start_pairing`
- `torca_engine_queue_message`
- `torca_engine_refresh_snapshot`

UTF-8 command arguments use explicit pointer + byte-length pairs. Dart obtains temporary argument buffers through `torca_alloc` and releases them through `torca_free`.

Command results and application snapshots are returned through native-owned UTF-8 JSON buffers using pointer/length getters. JSON is presentation-safe bridge data, not serialized domain aggregates.

## Result shape

```text
{
  ok: bool,
  kind: string,
  error: string?
}
```

## Snapshot shape

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

All identifiers remain canonical lowercase hexadecimal values and times remain bounded Unix milliseconds.

## Security and ownership rules

1. Flutter owns no private key bytes, SQL connections, Tor sockets or workflow state.
2. The ABI returns only bridge DTO data and redacted errors.
3. Native result/snapshot pointers are borrowed and remain valid only until the next mutating native call.
4. Platform-specific protected-key APIs remain behind Rust/platform composition boundaries.
5. `MemoryEngineGateway` is test/development-only and must be selected explicitly.
6. Failure to load the native library is surfaced to the user; production never silently falls back to memory state.

## Current 0.1 limitation

The shared ABI exists, but the current native constructor still starts `ClientEngine::default()`. Production SQLCipher repositories, RustCrypto and protected platform keys must be injected into the native composition before GATE-001/GATE-002/GATE-003 can be considered closed.
