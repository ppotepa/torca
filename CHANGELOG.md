# Changelog

Notable Torca changes are recorded here from this documentation baseline forward. The project is still pre-release; older development history remains in Git rather than being reconstructed as fictional releases.

## [Unreleased]

### Added

- Canonical product/build/compatibility versioning and release policy.
- A changelog workflow for future release notes.
- A single maintained `0.3` roadmap focused on UX/UI stabilization, broken-flow repair, responsive/accessibility quality and visual regression coverage.
- A framework-free interactive `0.3` UI maquette under `docs/maquettes/view/` with class-based screens, in-memory state, deterministic scenarios, responsive/theme/locale controls and a component UI Lab.
- Maquette parity surfaces for bootstrap/profile setup, local identity/about, peer connection details, Transfer Center and the production-style application overflow hierarchy.
- Ethernet-style provider/P2P `LINK`/`TX`/`RX` monitors plus a maquette-only `LIVE` telemetry simulator.
- Full maquette diagnostics cockpit with Battery, Runtime, Logs and Incident views, log filtering/pause, battery observation and mock incident/export flows.

### Changed

- Consolidated project documentation around one source of truth per concern.
- Expanded the architecture model to match the current layered Rust workspace and provider-neutral application boundary.
- Aligned product, transport, application-flow, security/privacy, development, testing and operations documentation with the Iroh-only production graph.
- Updated architecture/application/message-delivery diagrams to remove retired Tor/WebRTC production paths.
- Reframed `STATUS.md` around maturity and required evidence instead of volatile test-count snapshots.
- Expanded the maquette to preserve existing product controls such as Instant Contact, Radio, full Settings groups and connection-health details while still exploring the `0.3` visual direction.
- Made Modern and Terminal maquette families structurally distinct: restrained rounded Modern geometry versus square Terminal controls/avatars/cards, separate icon styling and the full current terminal palette set.
- Expanded the maquette semantic icon catalogue to mirror the production `TorcaIconSet` vocabulary instead of relying on one generic SVG set.

### Removed

- Retired alpha handoff documentation after durable conclusions were moved into canonical pages.
- Retired the completed/transitional Iroh CPU/battery plan from maintained documentation.
- Retired the obsolete WebRTC host-integration document from maintained architecture documentation.
- Removed the stale `services/relay` documentation placeholder; no active server implementation is maintained under `services/` in this checkout.

### Fixed

- Removed remaining active-document references that presented Tor as a current/selectable production path.
- Removed duplicated Iroh pairing-bootstrap wording and stale local README references.
- Restored network/activity, diagnostics, secondary-screen and settings capabilities that the first maquette iteration had accidentally simplified away.

### Security

- Clarified the boundary between content confidentiality/authentication and Iroh network-location metadata exposure.
- Clarified current non-guarantees, including lack of Signal-style forward secrecy/post-compromise security and protection after endpoint compromise.
