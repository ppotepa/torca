# Iroh alpha implementation audit

This report is retained as a dated validation record. The active architecture is Iroh-only in production; Memory is test-only. Tor, WebRTC and the external relay deployment are not part of the workspace.

## Verified gates

- Provider boundary check passes: production native graph resolves to Iroh.
- Workspace check, formatting, clippy and Rust tests pass after the provider cleanup.
- Pairing-service protocol/client use provider-neutral names and persisted contacts contain opaque routes.
- Route freshness, reconnect demand, availability projection and diagnostics remain covered by targeted tests.
- Soak runner records Iroh profile, workload, CPU and Android battery artifacts without hidden polling.

## External evidence still requiring hardware

Physical Android measurements, Wi-Fi/LTE migration and long-duration always/direct profile comparisons require a connected handset and controllable network. These are validation activities, not alternate production providers.
