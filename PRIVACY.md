# Torca privacy notice

Last updated: 2026-08-12

Torca is a peer-to-peer messenger designed to keep ordinary one-to-one message
delivery off a central messaging service. This notice describes the behavior of
the source in this repository. A distributor that changes the application or
adds services must update this notice for its build.

## Data stored on your device

Torca stores your local identity, contacts, conversations, delivery state,
settings, and attachment metadata in an encrypted SQLCipher database. Private
identity and storage secrets are kept through operating-system protected-secret
adapters. Attachments copied into Torca are kept in application-controlled
storage. Data remains until you clear the relevant history, remove a contact,
delete application data, or uninstall the application, subject to operating
system backup and deletion behavior.

Files that you explicitly export leave Torca-controlled storage. Torca cannot
control copies, screenshots, notifications, or exports retained by you, the
operating system, or the intended recipient.

## Data sent over the network

Normal paired-contact traffic is sent directly to the contact's onion service
through Tor and is authenticated and encrypted by Torca. The intended recipient
receives the content and metadata needed to display and deliver the message.

Pairing uses an ephemeral rendezvous relay. The relay may observe connection
timing, slot lifetime, and protocol metadata required to join two devices. It is
not the normal message path and is not intended to receive conversation content,
private identity keys, or durable conversation history.

Tor and the internet providers involved in a connection can observe traffic
patterns available at their respective network positions. Tor reduces direct
network-location exposure between peers but does not eliminate timing or traffic
correlation risk.

## Notifications and diagnostics

Notification content follows the in-app privacy setting. Operating-system
notification services can process the notification metadata displayed by the
device. The current source does not add advertising or application-analytics
telemetry. Diagnostics are designed to exclude message and attachment content;
they are shared only when a user deliberately exports or sends them.

## Choices

You can disable read receipts and notification content, clear conversation
history, remove contacts, and delete the application's local data. Blocking a
contact stops reconnection but does not erase copies already held by that
contact.

## Security and questions

No software can promise absolute confidentiality, and this alpha has not
received an independent production security audit. The precise guarantees and
non-guarantees are documented in [SECURITY.md](SECURITY.md). Privacy or security
questions should be raised with the repository maintainers; do not include live
keys, pairing capabilities, database keys, or private conversation content in a
public issue.
