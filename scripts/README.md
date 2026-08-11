# Development workflow

Use one interactive entry point for normal local work:

```powershell
.\scripts\wizard.ps1
```

The wizard always displays and uses every deployable Windows and Android device
for the selected client platforms. It exposes a small set of complete workflows:

- `Run current build` only launches the existing installed/built clients.
- `Redeploy current build` reinstalls the existing endpoint-compatible artifacts.
- `Rebuild selected components` lets you select Android, Windows and/or relay in
  one comma-separated answer. Client and relay data and the Onion address stay intact.
- `Full redeploy` rebuilds relay and all clients, resets client application data,
  but keeps the Onion identity and warm relay Tor cache.
- `Full redeploy + new Onion` is the explicit destructive network reset. It
  rotates the endpoint and therefore always rebuilds every client.
- Relay maintenance, status and all-device `logs.zip` collection are available
  from the same first-level wizard.

The wizard asks only for choices that affect the selected workflow and prints
one final execution plan. Destructive workflows require one confirmation word.
The remaining `.ps1` files are compatible automation and implementation
entrypoints; normal development does not require calling them directly.

The workflow can also be selected without navigating the menu, while still
using the same orchestration path:

```powershell
.\scripts\wizard.ps1 -Action Run
.\scripts\wizard.ps1 -Action Redeploy
.\scripts\wizard.ps1 -Action Rebuild -Components android,windows -Configuration debug
.\scripts\wizard.ps1 -Action FullRedeploy -Configuration release
.\scripts\wizard.ps1 -Action FullRedeployNewOnion -Configuration release
.\scripts\wizard.ps1 -Action Rebuild -Components relay
.\scripts\wizard.ps1 -Action Rebuild -Components onion
.\scripts\wizard.ps1 -Action Rebuild -Components android,relay -PlanOnly
```

`onion` is intentionally not treated as an ordinary rebuild component: it
means generating a new identity/address. Selecting it automatically includes
the relay and every connected client so no binary can retain the old embedded
endpoint.

Normal interactive examples:

```powershell
.\scripts\wizard.ps1
.\scripts\wizard.ps1 -Action Run
.\scripts\wizard.ps1 -Action Rebuild -Components android,windows
.\scripts\wizard.ps1 -Action FullRedeployNewOnion
```

The lower-level `build.ps1`, `run.ps1`, `deploy.ps1`, `redeploy.ps1` and
`torca.ps1` entrypoints are retained for CI and unattended automation. Calling
`deploy.ps1` without lifecycle arguments or calling `torca.ps1` without a
command redirects to this same wizard.

`stack rotate` intentionally creates a new onion identity. The generated
endpoint is stored in `.torca/stack/relay_endpoint.txt` and is passed as
`TORCA_RELAY_ENDPOINT` to the native build, so a build after rotation embeds
the new endpoint directly in both platform native libraries. `.torca` is
runtime state and is ignored by Git.

Full `deploy` defaults to `Ensure`: it preserves the current onion and creates
one only when none exists. Use `-OnionPolicy Rotate` when a fresh network and a
new client build are required. `-OnionPolicy Preserve` fails instead of
creating missing network state.

The interactive wizard first selects a lifecycle scope:

- `ClientsAndRelay` deploys clients while preserving their identity, encrypted
  database and persistent Arti directory cache by default.
- `RelayOnly` starts, restarts or repairs only the relay. It does not enumerate
  devices and cannot build, install, launch, reset or rotate the endpoint used by
  clients. Use this when the relay needs maintenance but client Tor sessions and
  warm caches must remain intact.
- `FullReset` explicitly erases all application data on selected clients before
  installation. This also erases the client Arti cache, so the next launch must
  perform a cold Tor bootstrap. Depending on the platform and Tor network this
  commonly takes 15-90 seconds or longer.

In non-interactive mode any client reset requires `-AllowDataReset`; this
prevents accidental replacement of the local identity and encrypted database.
The relay stack is protocol-health checked before a deploy is allowed to install
artifacts.

Relay maintenance has three non-destructive identity-preserving levels:

- `Ensure` keeps a healthy running relay as-is.
- `Restart` restarts it while preserving both onion identity and warm relay Tor
  directory cache.
- `Repair` preserves the onion identity but clears only the relay directory
  cache, forcing the relay to redownload consensus and microdescriptors.

The wizard's `Rebuild + repair relay` choice combines `Repair` with a forced
server build. It stops the relay (clearing all in-memory slots and connections),
keeps the HSS identity/security state required for the same onion address,
clears disposable directory cache and builds the current relay sources before
deploying clients. Select `Install: Always` to deploy every connected Android
and Windows device; this option now bypasses the per-device selector and really
uses all deployable devices.

`Rotate` is intentionally unavailable for `RelayOnly`, because changing the
onion address requires rebuilding and reinstalling clients with the new embedded
endpoint.

Build manifests are stored independently for each target and configuration:
`.torca/manifests/android-release.json` and
`.torca/manifests/windows-release.json`. The deploy orchestrator compares the
source fingerprint, endpoint, target, configuration, ABI and artifact before
building. An unchanged target therefore reuses its existing release build.

For unattended cache verification use, for example:

```powershell
.\scripts\deploy.ps1 -Target windows -BuildPolicy IfRequired -InstallPolicy Skip -RunPolicy Skip -NonInteractive
.\scripts\deploy.ps1 -Target android -BuildPolicy IfRequired -InstallPolicy Skip -RunPolicy Skip -NonInteractive
```

Collect the last diagnostic runs from the host and connected devices:

```powershell
.\scripts\collect.ps1 -LastRuns 10 -Target all -Profile extended
```

The collector writes a redaction-safe manifest, per-device logs and SHA-256
checksums into `logs/collected/<date>/collect-<run>`, together with a ZIP.
Android USB and ADB Wireless aliases for the same physical device are grouped
into one logical device. The `incident` profile is opt-in and may collect a
wider Logcat snapshot.

Quick operational helpers are available for repetitive local actions:

```powershell
# Install the last validated ABI APK without rebuilding.
.\scripts\redeploy.ps1 -Device 85Z5AIGU79XSLZMZ

# Repeat the last successful target/device/configuration without the wizard.
.\scripts\redeploy.ps1 -UseLast

# Fast developer iteration; rebuild only when the source fingerprint changed.
.\scripts\redeploy.ps1 -UseLast -Configuration debug -Validation Quick -BuildPolicy IfRequired

# Production-equivalent packaging and complete validation.
.\scripts\redeploy.ps1 -UseLast -Configuration release -Validation Full -BuildPolicy Rebuild

# Inspect deduplicated devices or full runtime status.
.\scripts\devices.ps1
.\scripts\devices.ps1 -Diagnostics

# Check the local toolchain and relay.
.\scripts\doctor.ps1 -Quick
.\scripts\doctor.ps1

# Manage the Tor relay stack.
.\scripts\stack.ps1 -Action status
.\scripts\stack.ps1 -Action restart
.\scripts\stack.ps1 -Action repair
.\scripts\stack.ps1 -Action rotate

# Tail relay or Android logs.
.\scripts\logs.ps1 -Source relay -Tail 200
.\scripts\logs.ps1 -Source android -Device 85Z5AIGU79XSLZMZ

# Stop runtime, or explicitly clear Android application data.
.\scripts\reset.ps1 -Device 85Z5AIGU79XSLZMZ -Scope Runtime
.\scripts\reset.ps1 -Device 85Z5AIGU79XSLZMZ -Scope All -Confirm

# Inspect generated release packages and clean selected generated data.
.\scripts\artifacts.ps1 -Action latest
.\scripts\clean.ps1 -Scope Flutter -Confirm
```

These helpers delegate to the existing modules and orchestrator. They do not
implement a second build or deployment pipeline. `clean.ps1` never removes
identity, database, or Arti state unless a future command explicitly adds
that scope.

Build validation has three explicit levels:

- `Full` runs formatting/codegen, architecture checks, workspace check, Clippy,
  Rust tests, Flutter analysis and Flutter tests. It is the release/CI gate.
- `Quick` checks metadata, architecture, generated contract, `torca-native` and
  Flutter analysis. It is intended for iterative debug redeploys.
- `Skip` omits compilation validation but still runs source/architecture policy;
  use it only when a matching artifact is already known to be valid.

Gradle build cache, parallel execution and Kotlin incremental compilation are
enabled. Cargo release builds retain incremental metadata. When `sccache` is on
`PATH`, the build automatically uses it as `RUSTC_WRAPPER`.

Android installation can still be refused by the device with
`INSTALL_FAILED_USER_RESTRICTED`; in that case enable ADB/USB installation on
the device and repeat deploy.

`build.ps1` defaults to Windows on Windows and to validation-only on other hosts. CI calls `build.ps1 -Target check -CI`.

Formatting, contract generation, architecture checks, toolchain checks, Cargo lock refresh, Clippy, tests, Flutter platform bootstrap, Android Rust cross-compilation, packaging, and checksums are implementation details in `scripts/modules/Torca.BuildEngine.psm1`. They are not separate developer workflows.
# Network stack providers

The stack can be provisioned through Docker Compose or through the Windows
process provider. In `auto` mode Docker is used only when the daemon responds;
if Compose cannot start, the script falls back to the process provider and keeps
that provider for subsequent runs until Docker is explicitly selected:

```powershell
.\scripts\torca.ps1 stack ensure -StackProvider docker
.\scripts\torca.ps1 stack ensure -StackProvider process
```

`-StackAction rotate` recreates the Docker Tor volume and therefore creates a
new v3 relay onion address. The endpoint is written to `.torca/stack/relay_endpoint.txt`
and is included in the next native build fingerprint. Use `-StackProvider process`
only for a local fallback when Docker Desktop is unavailable.
