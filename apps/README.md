# Applications

This directory contains user-facing deployable compositions. Applications assemble libraries; they do not define domain rules.

## Planned application

- [`client`](client/README.md) — shared Flutter client with Windows and Android hosts.

Application code may choose concrete adapters, initialize the ClientEngine, connect the generated bridge, configure logging and manage platform lifecycle. Reusable logic belongs in crates or shared Flutter packages.
