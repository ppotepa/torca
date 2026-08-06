# torca-receipts

## Purpose

Own user-level delivery and read receipt semantics separately from transport acknowledgements.

## Owns

- receipt identifiers and message references;
- delivered and read receipt kinds;
- monotonic transition rules;
- duplicate receipt handling;
- receipt creation and acceptance events;
- receipt repository and durable-send ports.

## Does not own

Peer frame acknowledgements, message content, notification APIs or database implementation.

## Key rule

A read receipt implies delivered state, but a transport write or peer protocol acknowledgement does not by itself imply user delivery.

## 0.1 completion

Delivered and read receipts are idempotent, reordered receipts converge to the highest valid state, and unsupported regressions are rejected.
