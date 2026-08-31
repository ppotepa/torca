# Torca security

Torca is security-sensitive alpha software. It has not received an independent production security audit and should not be presented as a finished high-risk communications product.

The detailed technical model lives in [`docs/security/THREAT-MODEL.md`](docs/security/THREAT-MODEL.md). This page summarizes security policy, current guarantees and explicit non-guarantees.

## Reporting a vulnerability

Do not publish reusable secrets, private user data or detailed exploit material in a public issue. Prefer GitHub private vulnerability reporting when it is available for the repository, or contact the repository maintainer through a private channel before public disclosure.

Include the affected commit/version, platform, reproduction conditions and the minimum sensitive material necessary to investigate. Remove message plaintext, private keys, relationship secrets, database keys and reusable pairing capabilities from diagnostics whenever possible.

## Current security properties

The current design intends to provide:

- explicit local approval before a pairing becomes a durable relationship;
- authenticated peer-session establishment bound to approved relationship material;
- application-layer authenticated encryption for peer content/control traffic;
- replay/deduplication protections appropriate to durable delivery/control paths;
- SQLCipher-backed structured local storage;
- platform-protected secret storage where supported;
- contact verification/identity-change handling; and
- privacy-bounded diagnostics and Android screen-capture protection by default.

Iroh is the sole production transport provider. Transport reachability does not replace Torca peer authentication or application-layer encryption.

## Explicit non-guarantees

Torca does **not** currently claim:

- Tor-style anonymity. Iroh direct/relay paths expose network metadata according to the path and infrastructure involved;
- Signal-style forward secrecy or post-compromise security in the current long-lived relationship-key design;
- protection after full compromise of the local device, OS or user account;
- prevention of copying, screenshots or recording by an authenticated recipient;
- guaranteed delivery/availability during device suspension, network loss or denial of service; or
- security certification merely because tests/builds pass.

A compromised long-lived relationship secret can therefore have consequences beyond one message/session.

## Security-sensitive change areas

Treat changes to these areas as requiring focused review and negative/failure tests:

- invitation/pairing capability parsing and approval;
- peer credentials/handshake/authentication;
- cryptographic key derivation/encryption/nonces/associated context;
- contact verification and identity change;
- protected secret/database storage and migrations/epochs;
- provider route/bootstrap behavior;
- attachments and Radio media/control protocols;
- notification privacy, diagnostics/log redaction and capture behavior; and
- generated/native/platform boundaries that can expose secret or durable state.

When a trust boundary or guarantee changes, update this page, the threat model and privacy documentation in the same change. Compatibility/security-visible changes also belong in [`CHANGELOG.md`](CHANGELOG.md).
