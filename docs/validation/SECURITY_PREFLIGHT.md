# Security preflight

`Validate-TorcaSecurity.ps1` is a repeatable release preflight, not a substitute for an independent security review.

Run:

```powershell
./scripts/Validate-TorcaSecurity.ps1
```

It executes the repository architecture/source/redaction/secret-lifetime policies, rejects suspicious tracked key-file names, records duplicate dependency versions, runs tests for the crypto, peer protocol, pairing protocol and SQLCipher crates, and runs `cargo audit` when it is installed. Results are written under `artifacts/security/`.

Before a production release, manually review at minimum:

- peer and pairing authentication/transcript binding, replay resistance and identity-change behavior;
- pairwise-secret storage and all temporary secret/key copies, including crash/error paths;
- SQLCipher key bootstrap, encrypted storage boundaries and backup/export behavior;
- logging/diagnostics/FFI responses for message content, paths, onion addresses, tickets, private keys and derived secrets;
- attachment encryption, staging/cache cleanup and integrity verification;
- relay/rendezvous trust assumptions, abuse/rate-limit boundaries and metadata exposure;
- JNI/C ABI pointer/lifetime/length validation and every `unsafe` block;
- dependency advisories and pinned protocol/crypto versions;
- protocol downgrade/version negotiation behavior;
- forward secrecy/post-compromise security status. The current pairwise-secret design must not be described as Double Ratchet/PCS unless that protocol is actually implemented and reviewed.

A release security sign-off should record commit SHA, toolchain versions, test output, advisory scan result, reviewer, unresolved findings and explicit risk acceptance. Device battery/connectivity soak results are separate release evidence and should be attached to the same sign-off package.
