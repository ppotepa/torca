# Naming and repository layout

## Product name

The user-facing and new source name is **Torca**. New Rust crates use the `torca-` prefix. Historical `torchat-*` names are not carried into the clean repository unless required temporarily by an imported protocol test vector.

## Planned layout

```text
apps/
  client/
crates/
  foundation/
  domains/
  application/
  infrastructure/
  protocol/
  platform/
services/
  relay/
docs/
  0.1/
  architecture/
  decisions/
```

## Crate naming

- domain: `torca-identity`, `torca-messaging`;
- application: `torca-client-engine`, `torca-projections`;
- infrastructure: `torca-storage-sqlite`, `torca-transport-tor`;
- protocol: `torca-wire`, `torca-peer-protocol`;
- platform: `torca-bridge`.

Names describe capability, not implementation layer aliases such as `helpers`, `common2`, `manager`, `new-runtime` or `misc`.

## Source layout inside a domain crate

```text
src/
  lib.rs
  model/
  commands/
  events/
  services/
  ports/
  error.rs
```

Use only the directories needed by that crate. Public exports are curated in `lib.rs`; internal modules remain private.

## Documentation

Every component directory starts with a README describing purpose, ownership, non-responsibilities, dependencies and completion criteria. Implementation-specific details move into source documentation once code exists.
