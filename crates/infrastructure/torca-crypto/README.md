# torca-crypto

## Purpose

Provide reviewed implementations of semantic cryptographic ports used by identity, pairing, messaging and attachments.

## Owns

- secure randomness;
- key generation and serialization formats;
- signing and verification;
- capability derivation and verification;
- payload sealing and opening;
- attachment encryption primitives;
- algorithm and format version identifiers;
- redaction-safe cryptographic errors.

## Does not own

Contact policy, message state, database keys at rest, platform keychain implementation or peer retry logic.

## 0.1 completion

All serialized cryptographic formats have test vectors, malformed inputs fail safely, secrets are zeroized where practical and no secret value implements unsafe diagnostic formatting.
