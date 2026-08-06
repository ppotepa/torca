# torca-peer-protocol

## Purpose

Define the versioned protocol exchanged between paired Torca clients after direct connection.

## Owns

- handshake challenge and response DTOs;
- authenticated session establishment messages;
- encrypted application-envelope wrapper;
- protocol acknowledgements;
- message, receipt and attachment payload DTOs;
- feature negotiation and strict limits.

## Does not own

Domain message objects, cryptographic operations, peer sockets, outbox policy or contact trust transitions.

## 0.1 completion

Version 1 test vectors cover handshake, text message, receipt, acknowledgement, malformed input and forward-compatible optional fields.
