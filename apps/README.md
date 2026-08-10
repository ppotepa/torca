# Applications

This directory contains user-facing deployable compositions. Applications assemble libraries; they do not define domain rules.

## Planned application

- [`client`](client/README.md) — shared Flutter client with Windows and Android hosts.

Application code starts the Flutter presentation worker and forwards platform lifecycle. Native runtime
composition, durable state, networking and logging are owned by Rust crates. Reusable logic belongs in
crates or shared Flutter packages.
