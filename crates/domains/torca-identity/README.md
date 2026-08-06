# torca-identity

## Purpose

Own the lifecycle and public representation of one local installation identity.

## Owns

- `IdentityId` and public identity value objects;
- local profile name and avatar reference;
- create, load and rotate identity operations;
- validity rules for public identity data;
- identity-created and identity-rotated events;
- repository and key-provider ports required by identity workflows.

## Does not own

Contacts, pairing sessions, peer connectivity, database schema, platform keychain APIs or cryptographic algorithm implementation.

## Planned public surface

```text
CreateIdentity
LoadIdentity
UpdateProfile
RotateIdentity
Identity
PublicIdentity
IdentityRepository port
IdentityKeyProvider port
```

## Allowed dependencies

Foundation types and narrow cryptographic contracts. No infrastructure crates.

## 0.1 completion

A new identity can be created once, persisted through ports, safely loaded after restart and represented publicly without exposing private key material.
