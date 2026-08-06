# Cryptographic contracts — Batch 06

`torca-crypto` defines redaction-safe semantic crypto interfaces and value types.

## Implemented

- private/public key, signature, nonce and ciphertext value types;
- redacted private-key diagnostics and best-effort zeroing;
- signing, verification, randomness and authenticated sealing/opening port;
- deterministic provider for tests and simulations.

## Security gate

`DeterministicTestCrypto` is explicitly insecure and must never be wired into a distributable client. Batch 06 remains partially complete until a reviewed production provider is integrated and test vectors are recorded.
