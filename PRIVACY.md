# Torca privacy notice

Last updated: 2026-08-25

Torca is a peer-to-peer one-to-one messenger designed to keep ordinary conversation delivery off a central messaging service. This notice describes the current source in this repository. A distributor that changes services, providers, telemetry or data handling must update the notice for its build.

Torca is alpha software and has not received an independent production security audit. See [`SECURITY.md`](SECURITY.md).

## Data stored on your device

Torca stores local identity references, contacts, conversations, message delivery/read state, settings, pairing workflow state and attachment metadata in SQLCipher-backed structured storage. Identity, storage, relationship and provider secret material is accessed through protected-secret adapters rather than exposed as Flutter presentation state.

Attachments imported into Torca are kept in application-controlled storage for transfer/history lifecycle. Explicitly opened/exported files can create plaintext copies outside Torca-controlled storage and are then governed by the destination application, operating system and user choices.

Radio Mode stores the durable consent/session/control state needed by the feature, including user-visible timeline state. Live microphone media is communication data rather than ordinary text history. An intended recipient can hear, record or otherwise retain audio after delivery.

Local data remains until the relevant product action removes it, application data is reset, or the application is uninstalled, subject to OS/filesystem/backup behavior.

## Data sent over the network

A deployment has one selected communication provider. Torca peer authentication and application-layer encryption remain active regardless of provider, but network privacy differs by provider.

### Tor provider

Tor uses embedded Arti/onion peer transport. This is intended to reduce direct network-location exposure between peers. New pairing sessions use an ephemeral managed rendezvous service that can observe operational metadata needed to run active pairing slots.

The managed rendezvous service is not the normal message/attachment/Radio path and is not intended to receive conversation plaintext, Radio media, identity private keys or durable conversation history.

### Iroh provider

Iroh uses QUIC/direct-path communication and direct QR/full-link pairing bootstrap. It does not require the managed Tor rendezvous service. Direct-path networking has a different metadata surface from onion routing; network participants/provider infrastructure may observe addressing/timing information available to that path.

Do not describe an Iroh deployment as hiding network location in the same way as a Tor deployment merely because message contents remain application-encrypted.

### Other adapters

WebRTC and memory adapters exist in source but are hidden from normal production deployment today. A future deployment that enables a new provider must document the provider's signaling/routing/metadata behavior here before making privacy claims.

## Communication content

Paired-contact traffic can include:

- ordinary messages and delivery/read receipts;
- attachment transfer/control traffic; and
- when mutually enabled and actively used, encrypted Radio Mode control/media traffic.

The intended recipient necessarily receives the content/protocol data required to render that communication. Radio transmission requires microphone access on the transmitting device.

## Pairing metadata

Pairing invitations/bootstrap material can be capability-bearing and should be treated as sensitive while active. Provider commissioning may expose connection timing/session metadata to the selected rendezvous/signaling/network service. Explicit approval and the encrypted pairing exchange establish the durable relationship; commissioning infrastructure is not the contact source of truth.

## Notifications, diagnostics and capture

Notification content follows the selected in-app privacy policy. OS notification services can process metadata/content displayed by the device.

Diagnostics are designed to exclude message/attachment plaintext, Radio audio, private keys, database keys, relationship secrets and reusable pairing capabilities. Diagnostic artifacts still contain operational metadata such as timestamps, build/device identifiers, provider state, endpoint summaries, errors and counters; share them only when needed.

The current source does not intentionally add advertising or application-analytics telemetry.

Android uses OS-level screen-capture protection by default. The deployment tool can explicitly allow screenshots/screen recording for local development. This changes the capture flag only; it does not change message encryption or provider routing.

## User choices

Depending on the current surface, users can control behavior such as read receipts, notification content, Radio consent and conversation/contact deletion. Blocking/removing a contact cannot erase copies already held by that peer.

A full local reset can remove identity, encrypted history and provider-specific local state, so deploy/development tools keep destructive reset separate from ordinary rebuild/redeploy.

## Privacy limits

Torca cannot prevent:

- an intended recipient from copying/recording/exporting content;
- a compromised endpoint/OS from accessing data available to that endpoint;
- provider/network operators from seeing metadata available at their network position;
- timing/traffic correlation in all threat models; or
- external apps/OS services from retaining user-exported files, notifications or captures.

The current relationship-key design also does not claim Signal-style forward secrecy/post-compromise security. See [`SECURITY.md`](SECURITY.md) and [`docs/security/threat-model.md`](docs/security/threat-model.md).

## Questions

For privacy/security questions, contact repository maintainers without placing live private keys, pairing capabilities, database keys, relationship secrets, private conversation content or Radio audio in a public issue. Follow the sensitive-reporting guidance in [`SECURITY.md`](SECURITY.md).