# torca-contract

The language-neutral wire contract is defined in `schema/torca_contract.json`.
The checked-in Dart projection is `schema/torca_contract.dart`; the Rust
contract boundary owns request/response DTOs and snapshot projections.
`tools/torca-contract-gen --check` rejects drift before compilation.
