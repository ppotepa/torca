# Torca documentation

This directory contains the maintained documentation for the current codebase. It deliberately avoids release-plan ledgers and completed implementation checklists; Git history is the archive for those records.

## Documentation map

| Document | Use it for |
| --- | --- |
| [`../README.md`](../README.md) | product overview, current capabilities and repository map |
| [`../ARCHITECTURE.md`](../ARCHITECTURE.md) | ownership, layering, runtime and provider composition |
| [`app-flows.md`](app-flows.md) | startup, pairing, messaging and diagnostics flows |
| [`transport.md`](transport.md) | Tor/Iroh/WebRTC/memory provider boundary and capabilities |
| [`development.md`](development.md) | local development and repository workflow |
| [`testing.md`](testing.md) | automated, platform, integration and soak validation |
| [`operations.md`](operations.md) | deploy/runtime lifecycle, diagnostics and recovery |
| [`STATUS.md`](STATUS.md) | concise current maturity and implementation status |
| [`../SECURITY.md`](../SECURITY.md) | security goals, guarantees and non-guarantees |
| [`security/threat-model.md`](security/threat-model.md) | assets, trust boundaries and threat review triggers |
| [`../PRIVACY.md`](../PRIVACY.md) | current data handling and provider-dependent network privacy |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | contribution and documentation rules |

## Diagrams

- [`diagrams/architecture.svg`](diagrams/architecture.svg) — client/runtime/provider system view.
- [`diagrams/app-flows.svg`](diagrams/app-flows.svg) — startup, pairing and conversation entry paths.
- [`diagrams/message-delivery.svg`](diagrams/message-delivery.svg) — outbound/inbound message ownership and feedback loop.

The SVGs are source-controlled documentation assets with embedded titles/descriptions and no external dependencies.

## Source-of-truth order

When documentation and implementation disagree:

1. checked-in source, generated contracts and tests;
2. enforced architecture/source policies;
3. maintained architecture/security/privacy documentation;
4. this documentation index/status material;
5. Git history for retired plans and validation ledgers.

A dated plan or old acceptance checklist is never evidence that the current binary passed that gate.

## Documentation rules

Document stable current behavior:

- responsibility and dependency direction;
- trust boundaries and security/privacy consequences;
- user/runtime flows;
- supported/selectable provider state;
- canonical build/deploy/test entry points; and
- the evidence required for a validation claim.

Avoid duplicating rapidly changing details such as exact schema numbers, migration counts, timeout constants, complete class inventories or test totals. Link to source when such details are the real contract.

When a temporary plan finishes, delete it from the maintained docs after moving any durable conclusion into the appropriate current-state page. Git history preserves the plan without forcing future readers to decide whether it is still authoritative.