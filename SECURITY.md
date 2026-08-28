# Torca security

Torca is security-sensitive alpha software and has not received an independent production security audit.

The sole production communication provider is Iroh. Direct paths can expose network-location metadata to the peer; configured Iroh relay paths change that exposure but do not provide Tor-style anonymity. Peer authentication, pairing approval, application-layer encryption, SQLCipher storage and protected-secret ownership remain Torca responsibilities.

The current message-key design does not claim Signal-style forward secrecy or post-compromise security. A compromised device can access local plaintext and keys, recipients can copy delivered content, and availability remains best effort under suspension, network changes and denial of service. Pairing-service infrastructure sees only operational metadata needed for an active pairing exchange.

See [`docs/security/threat-model.md`](docs/security/threat-model.md) for the detailed asset and boundary analysis.
