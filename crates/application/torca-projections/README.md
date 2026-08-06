# torca-projections

## Purpose

Build immutable, presentation-oriented snapshots from authoritative application state.

## Owns

- app startup and readiness snapshot;
- contact-list items;
- conversation summaries;
- paged message view models;
- pairing progress view;
- health and diagnostics projection;
- monotonic snapshot revision.

## Does not own

Flutter widgets, database connections, domain mutation or navigation state.

## Rules

Projection models may flatten multiple domains for reading, but they cannot become writable domain objects. Sensitive fields are excluded by default.

## 0.1 completion

Windows and Android consume the same generated projection contracts and can rebuild their UI after subscription restart using the latest complete snapshot.
