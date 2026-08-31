# Tests

Torca validates behavior at several layers rather than treating one end-to-end suite as the only source of confidence.

The Rust workspace contains focused domain/protocol/application/infrastructure tests. `tests/torca-integration` contains cross-component journeys spanning multiple crates. Flutter has widget/contract behavior tests, while supported-platform builds and device/soak scenarios provide different evidence.

Prefer the lowest layer that reproduces a behavior deterministically. Use controlled/fake adapters for retry, failure, restart and state-machine scenarios. Use real Windows/Android/Iroh network/device validation when the behavior depends on actual OS, reachability, background execution or power behavior.

Memory-provider tests do not prove Iroh reachability, and emulator CPU does not prove physical Android battery usage.

No documentation/change report should claim a platform/release gate passed unless it was actually executed for the referenced source/artifact.

See [`../docs/TESTING.md`](../docs/TESTING.md) for evidence terminology and the maintained validation matrix.
