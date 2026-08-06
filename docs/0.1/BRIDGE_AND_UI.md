# Generated bridge and shared Flutter shell — Batches 15–16

Implemented:

- versioned Rust bridge command/result/snapshot DTOs;
- deterministic Rust-to-Dart contract generation and stale-output validation;
- EngineBridge mapping to typed ClientEngine commands;
- shared Flutter gateway abstraction and memory preview adapter;
- identity setup, pairing, contacts, conversation and diagnostics routes;
- snapshots as the source of workflow state; Flutter owns only presentation controls.

The memory gateway is a UI preview adapter, not the production runtime. Platform builds must compose the generated contract with the native Rust library.
