# Torca project status

Torca is security-sensitive alpha software. Iroh is the only production
communication provider; Memory is retained as a deterministic test provider.

The current workspace validates provider-neutral routing, Iroh peer transport,
pairing persistence, durable storage and runtime policies. Automated Rust tests,
workspace clippy and the provider-boundary check are the primary gates.

Remaining alpha validation is real-device Windows/Android soak testing: pairing,
restart without re-pairing, Wi-Fi/LTE route changes, background/foreground
delivery, receipts, attachments and battery measurements. Passing local tests is
not an independent security audit or a guarantee of battery usage.
