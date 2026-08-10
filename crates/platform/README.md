# Platform libraries

Platform libraries expose stable application contracts to Flutter and native hosts.

Current components:

- [`torca-contract`](torca-contract/README.md): canonical operation metadata and contract-drift checks.
- `torca-native`: the process runtime registry, minimal C ABI and Android JNI boundary.
- `torca-platform`, `torca-platform-windows`, `torca-platform-android`: operating-system services and
  protected-secret/path adapters.

Platform code is an adapter. It must not reimplement pairing, messaging, retries or persistence.
