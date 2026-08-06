# Identity domain — Batch 04

`torca-identity` owns one local installation identity, its public projection, profile and key-generation continuity.

## Implemented

- typed `IdentityId` and `KeyId`;
- validated profile name and avatar reference;
- public identity without private key material;
- create-once, load, profile update and key rotation workflows;
- optimistic generation checks;
- repository and key-provider ports;
- in-memory repository and deterministic test key provider.

Private key bytes never enter the identity model. A production key provider remains an infrastructure concern.
