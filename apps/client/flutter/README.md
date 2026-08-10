# Torca Flutter client

This directory is the single Torca application client.

## Responsive composition

There is no desktop UI implementation and mobile UI implementation to keep in sync. The shared widget tree adapts by available width:

- compact layouts use normal routed screens;
- wide layouts render a conversation list and the same `ConversationPane` side by side;
- all commands go through the same `EngineGateway`; root state comes from bounded Rust snapshots and
  conversation history comes from paginated Rust queries.

The default production gateway is `FfiEngineGateway`, backed by the shared `torca_native` Rust library. Test-only fakes live under `test/`; production never falls back to an in-memory engine when native startup fails.

`NativeRuntimeWorker` owns the only presentation handle and serializes FFI calls. It currently polls a
bounded root snapshot and notification cursor; it must not derive business state from cached message
history. A Rust event-journal transport is planned as a replacement for frequent polling.

## Platform targets

`windows/` and `android/` are standard Flutter platform scaffolds generated automatically when required by `build.ps1`, `run.ps1` or `deploy.ps1`. They are build products rather than alternative application source trees.

Android-specific security/lifecycle overlays are applied from `tools/build/overlays/android` after the standard Flutter scaffold is generated.

## Supported SDK

- CI Flutter baseline: `3.44.7`
- local minimum Flutter: `3.44.0`
- Dart minimum: `3.12.0`

## Workflow

From the repository root:

```powershell
./scripts/build.ps1
./scripts/run.ps1 -Target windows
./scripts/run.ps1 -Target android -Device emulator-5554
./scripts/deploy.ps1 -Target all
```
