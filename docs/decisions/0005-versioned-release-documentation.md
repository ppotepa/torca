# ADR 0005: Versioned release documentation

- Status: accepted
- Date: 2026-08-06

## Context

A single evergreen roadmap tends to mix current commitments with distant ideas. Torca needs a precise reference for the version currently being implemented.

## Decision

Release-specific scope, roadmap, status and completion criteria live under `docs/<version>/`. Long-lived architecture rules remain under `docs/architecture`.

Only the active version receives detailed implementation planning. Version 1.0 remains an eventual goal, not a current scope container.

## Consequences

- implementation work has an unambiguous reference;
- completed release documentation remains historical evidence;
- architecture and release planning do not overwrite each other;
- maintainers must update links when a new active version begins.
