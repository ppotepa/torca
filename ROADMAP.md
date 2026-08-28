# Torca roadmap

This roadmap describes long-lived product and engineering direction. It is deliberately not a release checklist: current implementation and validation handoff lives in [`docs/STATUS.md`](docs/STATUS.md).

## Direction

Torca is moving toward a dependable private one-to-one messenger with direct authenticated peer communication over Iroh.

The desired end state is a product where a user can install the same Torca application on a supported platform, create a local identity, explicitly pair with another person, and communicate reliably without depending on a central account/history/message service.

## Near-term priorities

### Reliability and device evidence

Reliability remains the first product feature. Continue hardening and measuring:

- bootstrap and degraded/offline behavior;
- durable outgoing work, retry and restart recovery;
- peer reconnect and slow/unavailable Iroh behavior;
- pairing recovery and explicit error states;
- attachment resume/cancel/export behavior;
- Windows/Android background/lifecycle behavior;
- Radio Mode permission, route-change, backgrounding and reconnect behavior;
- battery/runtime behavior under real idle and active traces; and
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
- privacy-conscious notifications/settings; and
- complete localization of user-facing surfaces.

Radio Mode should remain clearly identified as an experimental paired-contact feature until its real-device acceptance and battery/recovery behavior are well characterized.

### Security maturity

Continue reducing the gap between an encrypted private messenger and a security-hardened messenger:

- keep identity/contact verification understandable to users;
- strengthen endpoint/secret handling and security-sensitive platform behavior;
- continue metadata minimization in logs, notifications and connectivity projections;
- review protocol/state-machine failure handling and malformed-input behavior;
- add/maintain dependency and advisory scanning appropriate to the Rust/Flutter supply chain;
- establish a clear private vulnerability-disclosure channel;
- evaluate a reviewed session/message key-evolution design before claiming forward secrecy or post-compromise security; and
- obtain independent security review before making strong production/high-risk-use claims.

The project should not invent a custom ratchet merely to advertise a feature.

### Performance and bounded state

As histories and contact lists grow, normal runtime/presentation paths should remain bounded:

- prefer event/deadline-driven work over fixed idle polling;
- use real transport evidence before scheduling health probes;
- keep message history paged;
- maintain efficient conversation summaries/unread projections;
- keep notification/connectivity/energy diagnostics payload-free and incremental; and
- avoid work proportional to complete message history in background hot paths.

Aggressive Iroh dormancy should remain measurement-gated. Do not trade reliable inbound/pairing/delivery recovery for an unmeasured battery optimization.

### Development ergonomics

Maintain one coherent codebase:

- keep architecture/source policies executable;
- keep application boundaries explicit and platform code thin;
- keep generated presentation contracts deterministic;
- reduce accidental duplication between Rust and Flutter;
- keep `torca-deploy` as the canonical build/run/deploy/log path;
- retire legacy compatibility helpers once Rust replacements are soak-tested;
- favor focused Rustdoc/central documentation over drifting mini-documents; and
- consolidate crate boundaries when they no longer provide meaningful dependency/ownership isolation.

## Later product areas

Features that may become appropriate after the core is reliable and secure include richer stored media/voice-note workflows, disappearing messages, message editing, reactions, archive/pin/mute ergonomics, richer privacy controls, groups, calls, multi-device support and additional platform hosts.

These are not promises for a particular release. Each should be designed so it does not reintroduce a central trusted message/history service unless the product model is deliberately changed.

## Non-goals for the current architecture

Torca is not currently aiming to become:

- a centralized social network or public identity directory;
- a cloud-first conversation/history service;
- a system where the pairing relay becomes the normal message/media path;
- a separate implementation of business logic per operating system;
- a plugin framework or generic distributed-systems platform; or
- a collection of feature-specific state-management/timer frameworks.

The architecture may evolve, but changes to these principles should be explicit product/architecture decisions rather than accidental consequences of adding a feature.
