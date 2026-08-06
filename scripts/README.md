# Development scripts

Torca intentionally exposes only three developer commands:

- `./scripts/build.ps1` — format/codegen, validate Rust and Flutter, and optionally build a platform client.
- `./scripts/run.ps1` — fast developer loop: prepare the shared native runtime and launch the client.
- `./scripts/deploy.ps1` — strict release build, package artifacts, write SHA-256 checksums, and optionally install Android output.

Examples:

```powershell
./scripts/build.ps1
./scripts/build.ps1 -Target android
./scripts/run.ps1 -Target windows
./scripts/run.ps1 -Target android -Device emulator-5554
./scripts/deploy.ps1 -Target all
```

`build.ps1` defaults to Windows on Windows and to validation-only on other hosts. CI calls `build.ps1 -Target check -CI`.

Formatting, contract generation, architecture checks, toolchain checks, Cargo lock refresh, Clippy, tests, Flutter platform bootstrap, Android Rust cross-compilation, packaging, and checksums are implementation details in `tools/build/Torca.Build.psm1`. They are not separate developer workflows.
