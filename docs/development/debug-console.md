# Debug console and incident evidence

The diagnostics surface is available only in debug builds. It exists to explain
runtime activity without turning production UI into an engineering console.

The target sections are:

- **Battery** — host energy facts, effective policy, leases, next wake and
  observation deltas.
- **Runtime** — Tor/onion/relay, actor liveness, queues and typed deadlines.
- **Logs** — bounded, redacted structured log tail with filtering.
- **Incident** — mark an incident and export a bounded support bundle.

Do not render unbounded logs in Flutter. Do not expose message plaintext,
attachments, Radio audio, pairing capabilities, private keys or relationship
secrets in the console or any export.

The canonical cross-host collector remains:

```powershell
cargo run -p torca-deploy -- logs --target all
```

See [`../diagnostics.md`](../diagnostics.md) for the bundle layout and sharing
rules.

