# Platform hosts — Batches 17–18

Implemented baseline contracts:

- shared Rust lifecycle policy for Windows and Android;
- Windows host manifest specifying one instance, tray close behavior and native library name;
- Android manifest with singleTask activity, disabled backup and required network/notification/service permissions;
- Flutter UI is platform-neutral and uses one generated bridge contract.

Remaining validation/composition work:

- generate native Flutter runner scaffolding with the pinned SDK;
- build and package `torca_bridge.dll` and Android ABI-specific `libtorca_bridge.so`;
- implement/verify Windows tray plumbing and Android foreground runtime ownership;
- run lifecycle and process-recreation tests on real platforms.
