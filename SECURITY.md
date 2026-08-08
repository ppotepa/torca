# Security policy

Torca 0.2 is an experimental test release and must not be represented as independently audited or production-safe.

Do not report sensitive vulnerabilities in public issues. Contact the repository owner privately with the affected version, reproduction steps, impact and any proposed mitigation. Do not include real private keys, peer secrets, pairing capabilities, message plaintext or database keys.

Security-sensitive changes require focused tests, threat-model updates and explicit review of logs, bridge DTOs, persistence and wire compatibility.

## Cryptographic scope of 0.2

Torca authenticates peers and protects peer payloads with authenticated encryption using a protected pairwise secret established during authenticated pairing. The 0.2 transport does not currently implement MLS or a Double Ratchet-style per-message key schedule, so forward secrecy and post-compromise security are not claimed for message history. Any future ratchet/MLS work must use a reviewed standard design rather than a custom protocol.
