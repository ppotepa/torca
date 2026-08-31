# Torca privacy notice

Torca is an alpha peer-to-peer messenger. This page describes the current data-flow/privacy model; it is not a claim of anonymity or protection after endpoint compromise.

## Local durable data

Rust-owned application data such as local identity references, contacts, conversations, delivery/read state, pairing state, runtime settings and attachment metadata is stored through SQLCipher-backed structured storage.

Secret material such as private identity/database/relationship secrets uses protected platform storage where supported. Presentation-only preferences can use normal platform/application preference storage and should not be treated as a secret store.

Application-managed attachment state/files are handled separately from structured SQLCipher data. Explicit export/open actions can create files outside Torca-controlled storage.

## Network data and metadata

Established paired traffic uses authenticated Iroh peer sessions plus Torca application-layer authenticated encryption. Depending on the active Iroh path/profile:

- a direct path can reveal network-location metadata to the paired peer and relevant network observers; and
- an Iroh relay path changes reachability/endpoint exposure but does not provide Tor-style anonymity or eliminate timing/volume metadata.

Commissioning/pairing infrastructure can observe operational timing/protocol metadata needed for the active exchange. It is not the normal conversation mailbox and must not receive Torca private keys or conversation plaintext as part of the intended protocol.

## Diagnostics and validation artifacts

Diagnostics are designed to expose operational state rather than user payloads. They must not intentionally include message/attachment plaintext, Radio audio, private identity keys, database keys, relationship secrets or reusable pairing capabilities.

Collected logs, screenshots, performance traces and soak bundles can still contain sensitive metadata such as device/build/network timing/state. Treat them as sensitive engineering artifacts and redact before sharing.

## Notifications, screenshots and external copies

Operating systems can display/store notification content according to Torca/OS settings. Android secure-window protection is enabled by default, but development builds can explicitly allow capture.

Recipients and operating systems remain outside Torca's control after data is displayed, played, recorded, screenshotted or exported. Deleting local data cannot delete copies already held by a recipient or external application/path.

## Reset and retention

A client-data reset is a destructive local operation and can remove identity, relationships, encrypted history and provider state from that installation. It is not a remote-revocation or recipient-deletion mechanism.

Torca does not currently include product-scope cloud backup or multi-device synchronization. Any future centralized analytics, backup, discovery or account service would change this privacy model and must be documented before production use.

See [`SECURITY.md`](SECURITY.md) and [`docs/security/THREAT-MODEL.md`](docs/security/THREAT-MODEL.md) for guarantees, non-guarantees and trust boundaries.
