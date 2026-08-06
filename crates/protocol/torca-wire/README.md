# torca-wire

`torca-wire` provides generic, versioned and strictly bounded binary framing shared by Torca protocol families.

## Status

Batch 03 implementation is complete. Owner-run local validation remains pending.

## Owns

- non-zero protocol-family and message-kind identifiers;
- major/minor compatibility policy;
- fixed version-1 wire header;
- stable envelope and correlation identifiers;
- strict frame and payload limits;
- deterministic encoding and decoding;
- incremental decoding of partial and concatenated frames;
- malformed-input and compatibility error taxonomy.

## Does not own

- domain models or domain events;
- messaging, pairing, relay or peer payload schemas;
- encryption, signatures or key material;
- transport sockets, Tor lifecycle or retry policy;
- database serialization;
- negotiation state machines.

## Version-1 framing

Every frame starts with a fixed 52-byte header followed by exactly the declared payload length.

```text
0..4    magic: "TRCA"
4       header version
5       flags
6..8    protocol family
8..10   protocol major version
10..12  protocol minor version
12..14  message kind
14..16  reserved, must be zero
16..20  payload length, big-endian u32
20..36  envelope identifier
36..52  correlation identifier
52..    payload
```

The default payload limit is 4 MiB. The configurable hard safety ceiling is 256 MiB. Lengths are validated before payload allocation.

## Compatibility rule

A codec accepts one configured protocol family, one major version and minor versions from zero through a configured maximum. Another family, another major version or a newer minor version is rejected explicitly.

Unknown message-kind behavior is controlled by the `REQUIRED_KIND` flag and interpreted by the family-specific protocol crate.

## Validation

Run from the repository root:

```powershell
./scripts/validate.ps1 -SkipFlutter
```

The package includes tests for deterministic round trips, older compatible minor versions, unsupported versions, oversized declared payloads, one-byte streaming input, concatenated frames and malformed magic.
