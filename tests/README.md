# Cross-component tests

This directory is reserved for tests that do not naturally belong to one crate.

Planned suites:

- two-engine pairing integration;
- two-engine transport-independent messaging;
- relay protocol integration;
- local Tor peer integration;
- restart and crash-boundary recovery;
- generated bridge compatibility;
- Windows and Android end-to-end journeys;
- migration fixtures and protocol test vectors.

Unit and port contract tests remain beside the code they validate. Cross-component tests must use public contracts and avoid reaching into private implementation modules.
