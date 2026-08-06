# ADR 0001: Modular monolith with mini-domain libraries

- Status: accepted
- Date: 2026-08-06

## Context

The previous codebase accumulated overlapping runtimes and migration layers. The new repository needs strong boundaries without operational complexity or unnecessary distributed services.

## Decision

Torca will be a modular monolith. Every meaningful mini-domain is represented by a focused Rust library. Deployable units compose these libraries in-process.

A separate crate is justified when an area has its own vocabulary, invariants, lifecycle, public contract and independent tests. Individual entities or helper functions do not receive separate crates.

## Consequences

- boundaries are visible and enforceable through Cargo dependencies;
- domains can be tested without infrastructure;
- deployment remains simple;
- cross-domain workflows require explicit application coordination;
- maintainers must resist both a giant runtime crate and excessive one-type crates.
