# ADR 0004: Main-only development

- Status: accepted
- Date: 2026-08-06

## Context

The repository starts empty and is being built in dependency order. Long-lived branches would quickly diverge from the architectural baseline and recreate migration work.

## Decision

The Torca baseline uses `main` as the active development branch. Work lands in small coherent commits directly on `main`.

## Consequences

- every commit must leave the repository consistent;
- incomplete work must be isolated and non-disruptive;
- status and architecture documentation are updated continuously;
- large speculative rewrites are discouraged;
- this decision may be superseded when contributor concurrency requires pull-request branches.
