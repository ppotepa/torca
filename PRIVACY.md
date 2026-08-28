# Torca privacy notice

Torca is an alpha peer-to-peer messenger. Local identity references, contacts, conversations, delivery state, settings, pairing state and attachment metadata are stored in SQLCipher-backed local storage. Secrets remain in protected platform storage where available.

Paired traffic uses authenticated Iroh peer sessions. Depending on the Iroh profile, traffic may use a direct path or an Iroh relay; direct paths can reveal network-location metadata to the peer. Pairing-service infrastructure can see timing and protocol metadata for an active pairing slot, but is not the normal message path and does not receive conversation plaintext or private keys.

Diagnostics exclude message plaintext, attachments, audio, private keys and pairing capabilities. Operating systems and recipients can retain screenshots, notifications, recordings or exported files outside Torca's control.

No software promises absolute confidentiality. See [`SECURITY.md`](SECURITY.md) and [`docs/security/threat-model.md`](docs/security/threat-model.md) for the current guarantees and threat model.
