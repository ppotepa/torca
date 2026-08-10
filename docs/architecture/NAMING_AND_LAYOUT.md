# Naming and repository layout

## Product name

The user-facing and new source name is **Torca**. New Rust crates use the `torca-` prefix. Historical `torchat-*` names are not carried into the clean repository unless required temporarily by an imported protocol test vector.

## Current layout

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
  0.2/
  architecture/
  decisions/
```

## Crate naming

- domain: `torca-identity`, `torca-messaging`;
- application: `torca-client-engine`, `torca-runtime`;
- infrastructure: `torca-storage-sqlite`, `torca-tor`;
- protocol: `torca-wire`, `torca-peer-protocol`;
- platform: `torca-contract`.

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

Component READMEs document stable public boundaries, ownership and non-responsibilities. Details that
are local to an implementation live in source documentation until that boundary is stable enough to be
documented independently.
