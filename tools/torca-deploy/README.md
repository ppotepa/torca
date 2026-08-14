# `torca-deploy`

The Rust deployment entry point replaces the growing family of PowerShell
entrypoints with one typed planner, one durable deployment checkpoint and two
interfaces over the same core:

```powershell
cargo run -p torca-deploy
cargo run -p torca-deploy -- status
cargo run -p torca-deploy -- plan --dry-run
cargo run -p torca-deploy -- rebuild --target all
cargo run -p torca-deploy -- resume
cargo run -p torca-deploy -- relay --dry-run rotate --confirm-rotate
cargo run -p torca-deploy -- logs --target all --dry-run
cargo run -p torca-deploy -- build --target windows --configuration debug
# Explicitly allow Android screenshots/screen recording for a local test run.
cargo run -p torca-deploy -- run --target android --privacy allow-capture
```

No command arguments opens the Ratatui wizard. CLI commands are intended for
CI and repeatable local automation. `--dry-run` validates and prints a plan
without starting Docker, Flutter, Cargo or ADB.

Rust owns orchestration and execution. It invokes Docker, Cargo, Flutter and
ADB through typed process adapters; the deploy path does not invoke PowerShell.
Each run is saved under:

```text
.torca/deploy/current.json
.torca/deploy/runs/<run-id>.json
.torca/deploy/runs/<run-id>.events.jsonl
```

The TUI asks for target, debug/release configuration, client-data policy,
onion policy and screen-capture privacy after the workflow is selected.
`Strict` is the default and keeps Android `FLAG_SECURE` enabled. The explicit
`Allow screenshots/recording` option only changes that Android window flag; it
does not change message encryption, transport privacy, or relay data handling.
`Enter` shows a final plan
confirmation. The
process output is streamed while the Rust checkpoint is updated after stages.

Relay onion rotation is destructive. The typed plan rejects it unless relay and
all client artifacts are rebuilt and both Windows and Android are targeted.
