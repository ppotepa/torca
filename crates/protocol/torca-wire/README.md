# torca-wire

## Purpose

Provide common framing, envelope headers, version identifiers and codec safety utilities for Torca protocols.

## Owns

- protocol family and version fields;
- message kind identifiers;
- bounded length-prefixed framing;
- correlation and envelope identifiers;
- common codec errors;
- compatibility helpers and test-vector format.

## Does not own

Domain models, message-state semantics, encryption keys, sockets or database serialization.

## 0.1 completion

Streaming decode handles partial frames, concatenated frames, oversized lengths, malformed headers and unsupported versions without panic or unbounded allocation.
