# Debug console and incident evidence

The diagnostics surface is available only in debug builds. It exists to explain
runtime activity without turning production UI into an engineering console.

The target sections are:

- **Battery** — host energy facts, effective policy, leases, next wake and
  observation deltas.
- **Runtime** — Tor/onion/relay, actor liveness, queues and typed deadlines.
- **Logs** — bounded, redacted structured log tail with filtering.
- **Incident** — mark an incident and export a bounded support bundle.

`Mark incident` is an explicit local command. It writes a redacted
`incidents/<incident-id>/` bundle below the current native log run, containing
`manifest.json`, `diagnostics.json` and bounded tails for the structured log
domains. It neither starts a network request nor schedules a background wake.
The normal cross-device collector can copy this bundle with the rest of the
native log tree.

Do not render unbounded logs in Flutter. Do not expose message plaintext,
attachments, Radio audio, pairing capabilities, private keys or relationship
secrets in the console or any export.

The canonical cross-host collector remains:

```powershell
cargo run -p torca-deploy -- logs --target all
```

See [`../diagnostics.md`](../diagnostics.md) for the bundle layout and sharing
rules.
