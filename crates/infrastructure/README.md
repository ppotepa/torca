# Infrastructure libraries

Infrastructure libraries implement ports required by domains and application workflows.

Planned components:

- [`torca-storage-sqlite`](torca-storage-sqlite/README.md)
- [`torca-crypto`](torca-crypto/README.md)
- [`torca-peer`](torca-peer/README.md)
- Tor transport and peer framing are owned by [`torca-tor`](torca-tor/README.md).
- [`torca-rendezvous-client`](torca-rendezvous-client/README.md)
- [`torca-file-storage`](torca-file-storage/README.md)

Concrete adapters are selected only in deployable compositions. Infrastructure types must not leak into domain public APIs.
