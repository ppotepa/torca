# Torca client

Torca has one client application: `apps/client/flutter`.

The same responsive Flutter source is used on every supported device. Windows and Android are platform targets generated and prepared by the private build tooling; they are not separate application implementations.

```text
apps/client/
└── flutter/
    ├── lib/       shared responsive UI and gateway
    ├── test/      shared widget/gateway tests
    ├── windows/   generated locally when a Windows target is built/run
    └── android/   generated locally when an Android target is built/run
```

Generated platform scaffolds are intentionally ignored by Git. Torca-specific Android system overlays live under `tools/build/overlays/android`; the build tooling applies them after Flutter creates the standard platform project.

The client owns presentation and presentation-worker startup/shutdown. Rust owns process-runtime
composition, messaging, pairing, contacts, persistence, cryptography, Tor protocol logic and retry
state machines.

Developer entrypoints from the repository root:

```powershell
./scripts/build.ps1
./scripts/run.ps1
./scripts/deploy.ps1
```
