# Torca 0.1 generic wire protocol

## Purpose

Batch 03 introduces only the common framing layer used by future peer, pairing and relay protocols. It deliberately does not define business payloads.

Implementation: `crates/protocol/torca-wire`.

## Header layout

Version 1 uses a fixed 52-byte big-endian header:

| Offset | Length | Field | Rule |
|---:|---:|---|---|
| 0 | 4 | magic | ASCII `TRCA` |
| 4 | 1 | header version | currently `1` |
| 5 | 1 | flags | unknown bits rejected |
| 6 | 2 | protocol family | non-zero |
| 8 | 2 | protocol major | non-zero |
| 10 | 2 | protocol minor | compatible range checked |
| 12 | 2 | message kind | non-zero, family scoped |
| 14 | 2 | reserved | must be zero |
| 16 | 4 | payload length | unsigned big-endian, validated before allocation |
| 20 | 16 | envelope ID | stable opaque 128-bit value |
| 36 | 16 | correlation ID | shared workflow correlation value |

The payload follows immediately after the fixed header. There is no padding or trailing checksum at this layer.

## Limits

- Default maximum payload: 4 MiB.
- Configurable hard ceiling: 256 MiB.
- Maximum frame length: fixed header plus configured payload limit.
- A decoder buffers at most one validated frame internally.
- Declared lengths above the configured limit fail before payload allocation.

Family-specific protocols may use lower limits but must not exceed the generic hard ceiling.

## Compatibility

A `WireCodec` is configured for exactly one protocol family and one supported major version. It accepts minor versions from zero through `max_minor`.

```text
same family + same major + received minor <= max minor  => compatible
other family                                           => reject
other major                                            => reject
newer minor                                            => reject
```

A future negotiation protocol may select the mutually supported version before normal frames are exchanged. Batch 03 does not implement that negotiation state machine.

## Message kinds and flags

Message kind zero is invalid. Concrete kind registries belong to family-specific crates.

The generic header currently defines one flag:

- `REQUIRED_KIND` — the receiver must reject the frame when the family-specific message kind is unknown.

Unknown header flag bits are rejected because their framing semantics cannot be assumed safely.

## Decoder behavior

`WireCodec::decode` distinguishes incomplete input from malformed input. It returns one frame and the number of consumed bytes, allowing callers to process concatenated frames.

`FrameDecoder` supports arbitrary chunk boundaries. It:

- accepts partial headers;
- accepts partial payloads;
- emits concatenated frames in order;
- validates a complete fixed header before reserving payload capacity;
- resets its partial state after malformed input;
- does not panic on malformed lengths or headers.

## Error classes

Encoding reports unsupported versions, oversized payloads and length overflow.

Decoding reports invalid magic, unsupported header version, unknown flags, non-zero reserved bits, invalid family/kind/version fields, unexpected family, unsupported protocol version, oversized payload, truncated exact input and trailing bytes.

## Security boundary

This layer provides framing safety, not authenticity or confidentiality. Encryption, signatures, capabilities, replay protection and peer authentication are added by later crypto and peer protocol layers.

## Deferred work

Not part of Batch 03:

- peer handshake messages;
- pairing or relay schemas;
- message and receipt payloads;
- encryption and authentication;
- version negotiation transport flow;
- generated cross-language contracts.
