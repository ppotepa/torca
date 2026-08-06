# Development scripts

This directory will contain supported repository entrypoints for validation, build, local infrastructure, deployment and diagnostics.

Planned commands should cover:

- repository validation;
- Rust and Flutter formatting and tests;
- dependency-boundary checks;
- generated contract verification;
- local relay and Tor stack startup;
- Windows and Android development deployment;
- diagnostic collection;
- release version consistency.

Scripts are thin orchestration layers. Product behavior must remain in libraries and deployable applications.
