# Protocol libraries

Protocol crates own bounded external representations, framing, version validation/negotiation and protocol-specific input limits used by Torca peers, pairing, pairing-service exchange, attachments and Radio.

Protocols are compatibility contracts rather than domain persistence models. Domain aggregates should not be serialized directly merely because they contain similar fields.

Protocol code remains independent from application orchestration, concrete infrastructure and platform/UI code. Architecture policy enforces this direction.

Exact versions, frame fields and byte layouts are implementation contracts documented by source/tests. Long-lived architecture prose should describe ownership and compatibility rules instead of copying every constant.

See [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) and [`../../docs/VERSIONING-AND-RELEASES.md`](../../docs/VERSIONING-AND-RELEASES.md).
