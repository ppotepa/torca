# Development scripts

Torca intentionally exposes only three developer commands:

For the full local deployment workflow use `torca.ps1`. It starts the local
relay and Tor Hidden Service, preserves the onion identity in `.torca`, detects
Windows and Flutter Android devices, and lets the operator choose rebuild,
installation, and restart policies from a menu.

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
./scripts/torca.ps1
./scripts/torca.ps1 status -NonInteractive
./scripts/torca.ps1 stack -StackAction rotate
./scripts/torca.ps1 deploy -NonInteractive -Target windows
```

`stack rotate` intentionally creates a new onion identity. The generated
endpoint is stored in `.torca/stack/relay_endpoint.txt` and is passed to the
existing platform-asset pipeline, so a build after rotation embeds the new
endpoint in the client. `.torca` is runtime state and is ignored by Git.

Full `deploy` defaults to `Ensure`: it preserves the current onion and creates
one only when none exists. Use `-OnionPolicy Rotate` when a fresh network and a
new client build are required. `-OnionPolicy Preserve` fails instead of
creating missing network state.

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

# Inspect deduplicated devices or full runtime status.
.\scripts\devices.ps1
.\scripts\devices.ps1 -Diagnostics

# Check the local toolchain and relay.
.\scripts\doctor.ps1 -Quick
.\scripts\doctor.ps1

# Manage the Tor relay stack.
.\scripts\stack.ps1 -Action status
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
.\torca.ps1 stack ensure -StackProvider docker
.\torca.ps1 stack ensure -StackProvider process
```

`-StackAction rotate` recreates the Docker Tor volume and therefore creates a
new v3 relay onion address. The endpoint is written to `.torca/stack/relay_endpoint.txt`
and is included in the next client build fingerprint. Use `-StackProvider process`
only for a local fallback when Docker Desktop is unavailable.
