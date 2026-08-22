# SOAK2 cockpit and balanced battery scenario

SOAK2 is the canonical end-to-end soak workflow. It deliberately separates
preflight from measurement:

```text
bootstrap/build → relay warm-up → Android deploy/permission → bot readiness
→ pairing reuse/provisioning → measurement baseline → balanced workload
→ evidence capture → controlled cleanup
```

Only the Ratatui cockpit is shown during an interactive run. Cargo, Docker,
ADB and linker output is routed to `Logs`; the bootstrap build is kept at
`.torca/soak/bootstrap/latest.log`. Use:

```powershell
.\scripts\soak.ps1 cockpit
```

The default active-messaging plan uses the isolated Android soak flavor
(`com.torca.torca_app.soak`) and five persistent bots. Bot
profiles live below `.torca/soak/bots/` and are never copied into the normal
Torca profile. A fresh pairing is only a provisioning event; subsequent runs
reuse existing contacts when the Android snapshot already contains the bot
set. The default workload is intentionally conservative:

* text activity is spaced at roughly two minutes;
* a 1 MiB attachment is queued infrequently;
* Radio is disabled unless `--radio` is explicit;
* battery baseline starts after setup and pairing, not during build/warm-up;
* ADB telemetry is sampled slowly during measurement to avoid waking the phone.

For multi-run/server provisioning, `tools/torca-soak-bot-host` is a separate
dev-only service. It supervises five `torca-lab-peer` processes, keeps durable
roots, and exposes only a loopback/token-authenticated JSON control surface.
The optional compose overlay is `infra/docker/compose.soak.yml`; it does not
change the relay container or store messages on the relay.

## Acceptance criteria

1. No build/compiler/Docker trace appears outside cockpit Logs.
2. `measurement_started` is recorded after all setup and before workload.
3. `manifest.json` records workload and whether Radio was enabled.
4. Battery artifacts contain `battery-start.txt`, `battery-end.txt`, and
   `batterystats.txt` from the measured window.
5. A cancelled run stops peers/relay and writes `run_cancelled`.
6. A normal app profile is never reset or reused as a bot profile, and the soak
   APK uses the isolated `.soak` application id.
7. `cargo test -p torca-soak -p torca-soak-bot-host --locked` is green.

The old `torca-battery-soak-tui` command is intentionally not a second entry
point. Use `torca-soak` or the PowerShell bootstrap.
