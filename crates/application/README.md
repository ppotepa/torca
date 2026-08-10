# Application libraries

Application crates coordinate Torca use cases and define the ports/read models that connect product semantics to concrete adapters.

The public presentation-facing facade is `torca-client-application`. It exposes application commands, queries, security projections and policy without requiring Flutter or the contract layer to know storage/network implementation details.

`torca-client-engine` is the single-writer consistency boundary for durable domain transitions. `torca-runtime` owns long-lived background coordination through application-defined drivers. Supporting crates isolate bootstrap state, connectivity, probing, delivery/control delivery, pairing coordination, communication supervision and diagnostics.

Application code may depend on foundation/domains/protocol contracts but must not import infrastructure or platform implementations. This dependency direction is checked automatically.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) for the maintained system description.