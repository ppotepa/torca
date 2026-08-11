# Applications

Torca currently has one product client: [`client/flutter`](client/flutter).

Windows and Android are platform hosts of the same Flutter/Rust application. Responsive UI differences belong in the shared widget tree; product workflows, durable state, networking and security stay in Rust.

Platform-specific host code is limited to real OS capabilities such as lifecycle, protected secrets, notifications, deep links, secure-window behavior and installation/device integration.

See the root [README](../README.md) and [ARCHITECTURE](../ARCHITECTURE.md) documents for the maintained system description.