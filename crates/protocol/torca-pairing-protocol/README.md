# torca-pairing-protocol

## Purpose

Define versioned opaque pairing payloads exchanged through rendezvous after a slot is established.

## Owns

- public identity offer DTO;
- onion endpoint and capability offer DTO;
- transcript binding fields;
- approval proof DTO;
- completion confirmation DTO;
- compatibility and size constraints.

## Does not own

Relay slot commands, pairing state transitions, contact creation or cryptographic provider implementation.

## 0.1 completion

Both roles derive and verify the same bound pairing result, and replaying payloads into another session fails transcript validation.
