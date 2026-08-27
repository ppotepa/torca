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
# Limit a repeatable Android deploy/soak to one exact ADB serial.
cargo run -p torca-deploy -- deploy --target android --device <adb-serial>
# Explicitly allow Android screenshots/screen recording for a local test run.
cargo run -p torca-deploy -- run --target android --privacy allow-capture
# Print the same normalized step graph used by the executor.
cargo run -p torca-deploy -- plan --dry-run --show-steps --preflight
# Select an alternate semantic color theme (or disable color).
cargo run -p torca-deploy -- plan --dry-run --theme amber --no-color
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

The TUI derives a contextual field list from `DeployPlan::capabilities()`.
Unsupported fields are hidden or disabled with a reason; implied values are
read-only. `DeployPlan::planned_steps()` is the single execution graph shown
by review and `--dry-run`. The semantic themes are `aurora` (default),
`amber`, and `high-contrast`; `NO_COLOR` and `--no-color` retain textual
status symbols while removing terminal colors. In the action screen, `t`
cycles themes and `c` toggles monochrome mode; the choice is persisted at
`.torca/deploy/ui.json`.
The CLI-only `--device` option restricts discovery, ABI selection, reset,
installation and launch to one exact device id; when omitted, all ready
devices for the selected target are used.
`Strict` is the default and keeps Android `FLAG_SECURE` enabled. The explicit
`Allow screenshots/recording` option only changes that Android window flag; it
does not change message encryption, transport privacy, or relay data handling.
`Enter` shows a final plan
confirmation. The
process output is streamed while the Rust checkpoint is updated after stages.
Each checkpoint includes a normalized plan fingerprint; resume rejects a
checkpoint whose plan was changed, preventing accidental reuse of stale
completed stages.

Relay onion rotation is destructive. The typed plan rejects it unless relay and
all client artifacts are rebuilt and both Windows and Android are targeted.
