# Tests

Torca uses tests at several layers rather than treating one end-to-end suite as the only source of confidence.

The Rust workspace contains focused domain/application/infrastructure tests. `tests/torca-integration` is the cross-component test package for coherent journeys that span multiple crates. Flutter has its own widget/contract behavior tests, and CI defines separate source/Rust, Flutter/contract and supported-platform build gates.

Prefer the lowest test layer that can reproduce a behavior deterministically. Use controlled/fake adapters for retry, failure, restart and state-machine scenarios; use real Windows/Android/Tor end-to-end validation for behavior that depends on actual OS/network integration.

No documentation statement should claim a release/platform gate passed unless it was actually executed.

See [`../CONTRIBUTING.md`](../CONTRIBUTING.md) for the normal validation workflow.