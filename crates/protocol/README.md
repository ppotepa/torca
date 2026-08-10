# Protocol libraries

Protocol crates own bounded wire formats, framing, version negotiation/validation and protocol-specific input limits used by Torca peers, pairing and the rendezvous relay.

Protocols are transport contracts rather than domain persistence models. Domain aggregates should not be serialized directly simply because they contain similar fields.

Protocol code must remain independent from application orchestration, infrastructure implementations and platform/UI code. The architecture policy enforces this dependency direction.

Exact versions, frame fields and byte layouts are implementation contracts documented by source/tests and should not be copied into long-lived architecture prose.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) for how protocol crates fit into the peer/pairing flows.