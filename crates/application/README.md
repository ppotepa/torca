# Application libraries

Application libraries coordinate workflows across mini-domains and expose client-facing projections.

Planned components:

- [`torca-client-engine`](torca-client-engine/README.md) — single-writer actor and composition boundary;
- [`torca-projections`](torca-projections/README.md) — immutable application snapshots for presentation.

Application code may depend on domain crates and port definitions. It must not embed SQL, cryptographic algorithms, wire codecs or Flutter widgets.
