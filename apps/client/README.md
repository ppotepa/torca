# Torca client application

## Purpose

Compose the shared Flutter presentation, generated bridge, Rust ClientEngine and platform adapters into Windows and Android applications.

## Current layout

```text
apps/client/
  flutter/              shared Flutter application baseline
  android/              Android host integration, added in Batch 18
  windows/              Windows host integration, added in Batch 17
```

The Flutter baseline is buildable and testable, but it intentionally contains no product workflow state. It exists to validate the workspace until generated bridge contracts and UI features are introduced.

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

## Validation

Run the canonical root command:

```powershell
./scripts/validate.ps1
```

## 0.1 completion

The Windows and Android builds initialize the same engine contracts, render equivalent snapshots and complete the same pairing and direct-message journey.
