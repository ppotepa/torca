# torca-contacts

## Purpose

Own the verified local relationship with a remote identity.

## Owns

- `ContactId` and remote identity reference;
- pending, active, blocked and removed relationship states;
- trust and verification metadata;
- contact display metadata controlled locally;
- activate, block, unblock and remove transitions;
- uniqueness rules for remote identities;
- contact repository port.

## Does not own

Current socket state, Tor health, message history, invitation transport, user presence or conversation rendering.

## Planned events

`ContactCreated`, `ContactActivated`, `ContactBlocked`, `ContactUnblocked`, `ContactRemoved`.

## 0.1 completion

Pairing can create one verified active contact idempotently, and invalid relationship transitions are rejected by pure domain tests.
