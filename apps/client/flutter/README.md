# Torca Flutter client

This directory is the shared Flutter application baseline for Torca 0.1.

Batch 01 intentionally contains only a minimal, testable application shell. Product workflows, bridge bindings, platform hosts, and feature screens are introduced by later batches after the Rust contracts exist.

## Supported SDK

- Flutter `3.44.7`
- Dart `3.12.0`

## Validate

From the repository root run:

```powershell
./scripts/validate.ps1
```

For Flutter-only validation:

```powershell
./scripts/validate.ps1 -SkipRust
```
