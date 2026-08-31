# Changelog

Notable Torca changes are recorded here from this documentation baseline forward. The project is still pre-release; older development history remains in Git and dated validation records rather than being reconstructed as fictional releases.

## [Unreleased]

### Added

- Canonical product/build/compatibility versioning and release policy.
- A maintained validation-evidence index separating dated reports from current project status.
- A changelog workflow for future release notes.
- A single maintained `0.3` roadmap focused on UX/UI stabilization, broken-flow repair, responsive/accessibility quality and visual regression coverage.

### Changed

- Consolidated project documentation around one source of truth per concern.
- Expanded the architecture model to match the current layered Rust workspace and provider-neutral application boundary.
- Aligned product, transport, application-flow, security/privacy, development, testing and operations documentation with the Iroh-only production graph.
- Updated architecture/application/message-delivery diagrams to remove retired Tor/WebRTC production paths.
- Reframed `STATUS.md` around maturity and required evidence instead of volatile test-count snapshots.

### Removed

- Retired alpha handoff documentation after durable conclusions were moved into canonical pages.
- Retired the completed/transitional Iroh CPU/battery plan from maintained documentation.
- Retired the obsolete WebRTC host-integration document from maintained architecture documentation.
- Removed the stale `services/relay` documentation placeholder; no active server implementation is maintained under `services/` in this checkout.

### Fixed

- Removed remaining active-document references that presented Tor as a current/selectable production path.
- Removed duplicated Iroh pairing-bootstrap wording and stale local README references.

### Security

- Clarified the boundary between content confidentiality/authentication and Iroh network-location metadata exposure.
- Clarified current non-guarantees, including lack of Signal-style forward secrecy/post-compromise security and protection after endpoint compromise.
