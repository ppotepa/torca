# Deterministic avatar architecture

## Invariants

- A local avatar is derived from a pseudonymous, OS-backed physical-device seed. Android uses
  app/signing-key-scoped `ANDROID_ID`; Windows uses `MachineGuid` with hostname fallback. The raw
  identifier never leaves the device. Reinstalling the application therefore reproduces the same
  genome even though Torca creates a new cryptographic identity.
- Rendered pixels are never part of pairing or SQLCipher persistence.
- The genome envelope is immutable, versioned and content addressed by SHA-256.
- Pairing carries the envelope only when a session is active. Completed pairing data keeps the
  remote envelope so the contact can render the same avatar after restart.
- Root snapshots expose descriptors only. Full genome payloads are fetched with `avatars.get`.
- Rendering and decoding are isolated from the UI thread and cached by
  `(genome_hash, size, animation, palette, renderer_version)`.

## Data flow

```text
OS device id -> pseudonymous SHA-256 seed -> AvatarRepository -> AvatarGenomeEnvelope
                         |              |
                         |              +-- cache: genome_hash/size
                         +-- UpdateProfile(avatarEnvelope)
                                      |
                         Rust runtime / SQLCipher
                                      |
                  pairing offer -> remote descriptor + envelope
                                      |
                         completed pairing persistence
                                      |
                  targeted avatars.get -> Flutter renderer
```

## Storage boundaries

- `avatar_genomes` is the shared immutable content-addressed object store.
- `local_avatar_genome` explicitly binds this device to its genome. It is never inferred from the
  most recently inserted genome.
- `contact_avatar_genomes` binds a paired remote identity/contact to its authenticated genome.
- `pairing_sessions.remote_avatar_*` stores the opaque remote envelope until the pairing is
  completed. Payloads are capped at 32 KiB and are validated before insertion or decoding.
- PNG sprite sheets may live only in the bounded platform cache; no PNG, SVG or platform bitmap
  is written to SQLCipher or sent over the wire.

## Versioning and rollout

`schema`, `generatorVersion` and `catalogVersion` are protocol fields. A renderer must reject an
unknown schema and may fall back to initials when a catalog is unavailable. A future generator
must keep old catalog versions readable; changing a catalog requires a new catalog version, not a
silent reinterpretation of an existing genome hash.

## Performance policy

- Generate the local genome once and persist the envelope.
- Coalesce concurrent requests for the same identity.
- Precompile horizontal sprite sheets in an isolate and retain a bounded memory cache (96 entries)
  plus a bounded disk cache (32 MiB). List and detail sizes share their content-addressed variants.
- Use one frame clock for all visible active avatars. `sleeping`, `blocked`, reduce-motion,
  background lifecycle and widgets outside `TickerMode` register no clock client.
- Use descriptors in list snapshots and load full payloads only for visible contact/avatar widgets.

## Presentation state

The UI resolves five orthogonal signals instead of conflating them in a single presence enum:

```text
presence  = online | away | offline | unknown
activity  = idle | typing | speaking | listening | sending | receiving
lifecycle = waking | active | sleeping
attention = none | unread | mention | incoming
condition = normal | reconnecting | error | blocked
```

Priority is `blocked > error > speaking > listening > incoming/mention > typing > transfer >
reconnecting/waking > unread > idle/sleeping`. `unknown` is never presented as proven online.

## Verification checklist

1. `cargo run -p torca-contract-gen -- --check apps/client/flutter/lib/generated/torca_contract.dart`
2. `cargo check --workspace`
3. Rust pairing, engine and SQLCipher tests
4. `flutter analyze` and `flutter test` in `apps/client/flutter`
5. `flutter test` in `packages/torca_avatar`
6. Manual restart test: pair, close both clients, reopen and verify identical avatars without
   regenerating or transferring rendered images.
