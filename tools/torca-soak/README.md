# Torca Iroh soak runner

`torca-soak` is the automated, event-driven runner for desktop and Android
validation. It launches the real Rust/Flutter runtime, provisions deterministic
bot contacts, records pairing/handshake/message/receipt events, and writes a
JSONL timeline plus summary under `.torca/soak`.

Only Iroh is accepted by the runner. Profiles are selected by scenario:
`always` for cross-network messaging, `direct` for low-overhead measurements,
and `local` for loopback laboratory runs. No external service, relay process or
legacy provider flag is required.

Examples:

```powershell
cargo run -p torca-soak -- --scenario runtime-lab --plain --duration-seconds 300
cargo run -p torca-soak -- --scenario idle-battery --android <serial> --plain
```

For CPU and battery work, run at least three repetitions, keep the device state
and screen policy constant, and compare the generated medians. The runner keeps
the workload deadline-driven so idle runs do not add polling traffic. Reports
must distinguish app CPU, transport activity, rendering/avatar work and Radio
Mode; a local percentage is not proof of battery drain without a power trace.
