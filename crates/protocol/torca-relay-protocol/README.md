# torca-relay-protocol

## Purpose

Define the minimal client-to-relay rendezvous protocol.

## Owns

- create-slot and join-slot requests;
- short-lived slot identifiers and public codes;
- opaque publish and receive frames;
- close, reject, expired and error responses;
- protocol version and maximum payload sizes.

## Does not own

Pairing cryptography, message delivery, user accounts, contact state, offline storage or relay deployment configuration.

## 0.1 completion

The protocol supports deterministic create/join/exchange/close behavior, rejects oversized payloads and reveals no requirement for the relay to parse pairing plaintext.
