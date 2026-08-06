# Messaging and receipts — Batch 10

Implemented:

- bounded text bodies and stable message IDs;
- inbound/outbound direction and explicit queued, sending, sent, delivered, read, failed and cancelled states;
- validated state transitions and delivery-attempt history;
- capped exponential retry policy;
- idempotent delivery/read receipts as a separate mini-domain;
- in-memory repositories and read-only conversation projections.

Receipt application is monotonic: duplicate facts are no-ops and read implies delivered.
