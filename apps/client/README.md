# Torca client application

## Purpose

Compose the shared Flutter presentation, generated bridge, Rust ClientEngine and platform adapters into Windows and Android applications.

## Owns

- application startup and shutdown composition;
- platform-specific entrypoints;
- loading platform-protected configuration;
- selection of concrete storage, crypto, Tor and notification adapters;
- Flutter routing and presentation composition;
- packaging metadata.

## Does not own

- messaging, pairing or contact rules;
- database queries;
- cryptographic algorithms;
- wire protocol encoding;
- retry state machines.

## Planned internal layout

```text
apps/client/
  flutter/              shared Flutter application
  android/              Android host integration
  windows/              Windows host integration
```

## 0.1 completion

The Windows and Android builds initialize the same engine contracts, render equivalent snapshots and complete the same pairing and direct-message journey.
