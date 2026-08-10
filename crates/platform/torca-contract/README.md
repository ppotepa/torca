# torca-contract

The canonical operation metadata is defined in `schema/torca_contract.json`.
The checked-in Dart projection is `schema/torca_contract.dart`; the generator derives the Rust operation
allow-list and verifies projection/version drift before compilation.

The current generator does not yet generate every request payload, response DTO or snapshot projection
from the JSON schema. Those handwritten wire models remain an explicit hardening task, documented in
[`docs/0.2/IMPLEMENTATION_ORDER.md`](../../../docs/0.2/IMPLEMENTATION_ORDER.md).
