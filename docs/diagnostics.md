# Diagnostic logging contract

Torca keeps producer storage separate and normalizes it only when a diagnostic
bundle is collected. Producers must never write into another producer's root.

## Producer roots

| Producer | Runtime location | Contents |
| --- | --- | --- |
| Windows client | `%LOCALAPPDATA%\Torca\logs\devices\<device>\<date>\run-*` | `run.start.json`, domain JSONL logs, `run.end.json` |
| Android client | app external files `torca/logs/devices/<device>/<date>/run-*` | the same structured client run format |
| Rust deployer | `.torca/deploy/runs` | durable run JSON and stage-event JSONL |
| Legacy PowerShell deployer | `%LOCALAPPDATA%\Torca\logs\devices\windows-host` | structured deployment runs |
| Relay | Docker logs plus `.torca/logs` compatibility files | live server output and persisted historical output |

The client logger owns the shared structured event envelope. A domain event is
one JSON object per line and includes schema, timestamp, level, run, device,
build, domain, component, code, message and redacted context. Platform logs such
as Logcat or `dumpsys` are evidence collected beside a client run; they are not
converted into application events.

## Canonical collection

Run from the repository root:

```powershell
.\scripts\zip.ps1 -Profile incident -LastRuns 20
```

The command collects every visible Windows and Android target into `logs.zip`.
Its expanded source remains under
`logs/collected/<date>/collect-<number>`. The archive layout is:

```text
sources/
  clients/windows/<host>/runtime/<logger-device>/<date>/run-*
  clients/windows/<host>/platform
  clients/android/<serial>/runtime/<logger-device>/<date>/run-*
  clients/android/<serial>/platform
  deploy/runs/{rust,windows-host}
  deploy/state
  relay/{live,persisted,state}
  host/discovery
```

`relay/live` is captured from the running container and is authoritative for
the collection time. `relay/persisted` is retained for historical comparison;
the collector reports its age and warns when it references another endpoint.

## Bundle integrity

Every bundle contains:

- `collection-manifest.json` with requested and discovered devices, source
  totals, repository identity, status, errors and warnings;
- `file-inventory.json` with every normalized source file and its size/time;
- `source-origins.json` for copied deploy/relay state;
- `checksums.sha256` covering the complete bundle;
- `collector-errors.jsonl` and `collector-warnings.jsonl`.

`status=complete` means every selected source was collected without an error.
`status=partial` is still a valid archive, but it must not be presented as a
complete incident capture. Missing ADB authorization, a client that has not
created its runtime log root, or inaccessible Android release logs are recorded
explicitly.
