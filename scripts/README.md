# Development scripts

Supported entrypoints:

- `./scripts/format.ps1` — format all Rust and Dart source.
- `./scripts/validate.ps1` — release metadata, architecture boundaries, generated contract, Rust and Flutter validation.
- `./scripts/check-release.ps1` — verify version/build/contract consistency.
- `./scripts/check-architecture.ps1` — detect forbidden domain dependencies and raw SQL outside storage.
- `./scripts/package.ps1 -Target windows|android|all` — validate, build release artifacts and write SHA-256 checksums.

Product behavior remains in libraries and applications; scripts only orchestrate tools.
