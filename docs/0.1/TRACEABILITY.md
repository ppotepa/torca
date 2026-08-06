# Torca 0.1 traceability

This matrix maps required product capabilities to their owning components and roadmap milestones.

| Capability | Primary owner | Supporting components | Milestone |
|---|---|---|---|
| Local installation identity | `torca-identity` | `torca-crypto`, `torca-storage-sqlite` | M2 |
| Local profile | `torca-identity` | projections, storage | M2 |
| Invitation creation and join | `torca-pairing` | rendezvous client, relay protocols | M3 |
| Explicit bilateral approval | `torca-pairing` | pairing protocol, crypto | M3 |
| Verified contact creation | `torca-contacts` | ClientEngine handler, storage | M3 |
| Direct conversation creation | `torca-conversations` | ClientEngine handler, storage | M3 |
| Text message creation | `torca-messaging` | conversations, projections | M4 |
| Durable outbound retry | ClientEngine application workflow | storage outbox, peer | M4/M5 |
| Inbound deduplication | storage contract with messaging acceptance | peer protocol | M4 |
| Delivered and read receipts | `torca-receipts` | messaging, peer protocol | M4/M5 |
| Authenticated peer session | `torca-peer` | peer protocol, crypto | M5 |
| Direct onion delivery | `torca-transport-tor` | peer, ClientEngine | M5 |
| Contact availability projection | `torca-presence` | peer observations, projections | M5/M6 |
| Shared client contract | `torca-bridge` | ClientEngine, projections | M6 |
| Windows composition | `apps/client/windows` | bridge and adapters | M6 |
| Android composition | `apps/client/android` | bridge and adapters | M6 |
| Privacy-safe notifications | `torca-notifications` | platform adapters | M6 |
| Encrypted image attachment | `torca-attachments` | crypto, file storage, peer protocol | M7 |
| Redacted diagnostics | application health projection | all adapters | M7 |
| Packaging and test release | application compositions | scripts and end-to-end tests | M8 |

## Change rule

When a capability changes owner, update this matrix, the relevant component README and an ADR if the dependency direction or architectural boundary changes.
