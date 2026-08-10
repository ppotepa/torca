# Torca roadmap

This roadmap describes product and engineering direction. It is deliberately not a version checklist: Torca changes quickly, while the long-lived priorities should remain understandable across refactors and release-number changes.

## Direction

Torca is moving toward a dependable private one-to-one messenger with a modern everyday user experience and direct peer communication over Tor.

The desired end state is a product where a user can install the same Torca application on a supported platform, create a local identity, explicitly pair with another person, and then communicate reliably without depending on a central account/history/message service.

## Near-term priorities

### Reliability

Reliability remains the first product feature. Continue hardening:

- bootstrap and degraded/offline behavior;
- durable outgoing work, retry and restart recovery;
- peer reconnect and slow/unavailable Tor behavior;
- pairing recovery and explicit error states;
- attachment resume/cancel/export behavior;
- background lifecycle on supported platforms;
- deterministic integration coverage for failure paths.

The UI should remain useful when network components are not ready. Local state/history should not be held hostage by Tor or relay availability.

### Everyday messaging UX

Keep improving the core one-to-one experience before expanding scope:

- fast conversation list and paged/searchable history;
- predictable unread/read semantics;
- message reply/retry/details workflows;
- attachment ergonomics;
- clear connection/bootstrap feedback without technical noise;
- responsive desktop/mobile interaction and keyboard/touch accessibility;
- privacy-conscious notifications and settings;
- complete localization of user-facing surfaces.

### Security maturity

Continue reducing the gap between "encrypted private messenger" and a security-hardened messenger:

- keep identity/contact verification understandable to users;
- strengthen endpoint/secret handling and security-sensitive platform behavior;
- continue metadata minimization in logs, notifications and connectivity projections;
- review protocol/state-machine failure handling and malformed-input behavior;
- evaluate a reviewed session/message key-evolution design when the project is ready to provide forward-secrecy/post-compromise-security guarantees;
- obtain independent security review before making strong production/high-risk-use claims.

The project should not invent a custom ratchet merely to advertise a feature.

### Performance and bounded state

As histories and contact lists grow, normal runtime/presentation paths should remain bounded:

- prefer cursor/event projections over full-state polling;
- keep message history paged;
- maintain efficient conversation summaries/unread projections;
- keep notification and connectivity paths payload-free and incremental;
- avoid work proportional to the complete message history in background hot paths.

### Development ergonomics

Maintain one coherent codebase:

- keep architecture policies executable;
- keep application boundaries explicit and platform code thin;
- keep generated presentation contracts deterministic;
- reduce accidental duplication between Rust and Flutter;
- keep build/run/deploy entrypoints small and reproducible;
- favor focused Rustdoc/central documentation over many drifting mini-documents;
- remove obsolete compatibility layers instead of indefinitely stacking new ones.

## Later product areas

Features that may become appropriate after the core is reliable and secure include richer media/voice-note workflows, disappearing messages, message editing, reactions, archive/pin/mute ergonomics, richer privacy controls, groups, calls, multi-device support and additional platform hosts.

These are not promises for a particular release. Each should be designed so it does not reintroduce a central trusted message/history service unless the product model is deliberately changed.

## Non-goals for the current architecture

Torca is not currently aiming to become:

- a centralized social network or public identity directory;
- a cloud-first conversation/history service;
- a system where the pairing relay becomes the normal message path;
- a separate implementation of business logic per operating system;
- a plugin framework or generic distributed-systems platform;
- a collection of feature-specific state-management frameworks in Flutter.

The architecture may evolve, but changes to these principles should be explicit product/architecture decisions rather than accidental consequences of adding a feature.