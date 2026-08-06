# torca-bridge

## Purpose

Generate and host the typed contract between Flutter and the Rust ClientEngine.

## Owns

- command DTOs accepted from Flutter;
- command result and typed error DTOs;
- immutable snapshot DTOs;
- health and diagnostic DTOs;
- serialization or FFI glue required by the selected bridge technology;
- deterministic Dart binding generation;
- compatibility tests between generated sides.

## Does not own

Domain entities as mutable UI models, database access, business validation, navigation or platform notification APIs.

## Rules

Generated output is never manually edited. Contract DTOs contain only bridge-safe values and exclude secrets. Every state-changing command carries a `command_id`.

## 0.1 completion

Windows and Android submit the same typed commands and consume the same snapshot schema without handwritten duplicate models.
