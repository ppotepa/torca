# Torca privacy notice

Last updated: 2026-08-14

Torca is a peer-to-peer messenger designed to keep ordinary one-to-one message delivery off a central messaging service. This notice describes the behavior of the source in this repository. A distributor that changes the application, adds services or changes data handling must update the notice for its build.

Torca is alpha software and has not received an independent production security audit. See [`SECURITY.md`](SECURITY.md) for the current security guarantees and non-guarantees.

## Data stored on your device

Torca stores local identity references, contacts, conversations, delivery/read state, settings, pairing workflow state and attachment metadata in SQLCipher-backed structured storage. Private identity, storage and relationship secrets are kept through operating-system protected-secret adapters rather than exposed to Flutter presentation state.

Attachments copied into Torca are kept in application-controlled storage for the transfer/history lifecycle. Explicitly exported files leave Torca-controlled storage and are then governed by the destination application, operating system and user choices.

Radio Mode stores the durable preference/session/control state needed by the feature, including user-visible timeline events. Live microphone media is handled as communication data rather than ordinary conversation-history text. The intended recipient can hear, record or otherwise retain audio after it is delivered.

Local data remains until the relevant feature clears/removes it, application data is deleted, or the application is uninstalled, subject to operating-system backup, filesystem and deletion behavior.

Torca cannot control copies, screenshots, recordings, notifications or exports retained by you, the operating system or an intended recipient.

## Data sent over the network

Normal paired-contact traffic is sent directly to the contact's onion service through Tor and is authenticated/encrypted by Torca at the application layer. The intended recipient receives the content and protocol metadata needed to deliver and render that communication.

This includes ordinary messages/receipts, attachment transfer traffic and, when Radio Mode is mutually enabled and actively used, live encrypted Radio Mode control/media traffic. Radio Mode requires microphone access on the transmitting device.

Pairing uses an ephemeral rendezvous relay. The relay may observe connection timing, slot lifetime and protocol metadata required to connect two pairing participants. It is not the normal message path and is not intended to receive conversation content, Radio Mode media, private identity keys or durable conversation history.

Tor and internet providers involved in a connection can observe traffic patterns available at their respective network positions. Tor reduces direct network-location exposure between peers but does not eliminate timing or traffic-correlation risk.

## Notifications, diagnostics and screen capture

Notification content follows the selected in-app privacy policy. Operating-system notification services can process the notification metadata/content displayed by the device.

Diagnostics are designed to exclude message/attachment plaintext, Radio Mode audio, private keys, pairwise secrets and pairing capabilities. Diagnostic bundles still contain operational metadata such as timing, device/build identifiers, errors and potentially onion endpoint state; share them only when needed.

The current source does not add advertising or application-analytics telemetry.

Android uses OS-level screen-capture protection by default. The development deployment tool can explicitly allow screenshots/screen recording for a local test run. That override changes the Android window capture flag; it does not change message encryption or network behavior. Content visible after an intentional capture override can be recorded by the operating system/user.

## Your choices

Depending on the current client surface, users can control privacy-related behavior such as read receipts, notification content, Radio Mode consent and conversation/contact deletion. Blocking a contact stops the relationship behavior defined by the application but cannot erase copies already held by that contact.

You can remove local application data through supported reset/uninstall flows. A full local reset can also remove identity, encrypted history and Tor cache state, so development/deployment tools treat destructive reset separately from ordinary redeploys.

## Security and questions

No software can promise absolute confidentiality. Torca does not currently claim Signal-style forward secrecy/post-compromise security for message history, and a compromised endpoint/OS can access data available to that endpoint.

For privacy or security questions, contact the repository maintainers without including live private keys, pairing capabilities, database keys, private conversation content or Radio Mode audio in a public issue. Follow the reporting guidance in [`SECURITY.md`](SECURITY.md) for sensitive findings.