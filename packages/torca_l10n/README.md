# torca_l10n

Shared localization package for Torca clients.

Each supported language has one complete ARB catalog in `lib/l10n`. The
English catalog is the key and placeholder source of truth. Generated Dart
sources live in `lib/src/generated` and are intentionally checked in so a
client can build from a clean checkout without depending on a local generator
cache.

Run the catalog gate from this directory:

```text
dart run tool/verify_catalogs.dart
dart run tool/audit_legacy_usage.dart
dart run tool/sync_legacy_symbols.dart
flutter gen-l10n
flutter test
```

The application depends on this package for the locale registry, generated
delegate, and all translated messages. The client contains no translation
source of its own.

`audit_legacy_usage.dart` verifies that every catalog has the same complete
key set. The deploy validation fails on incomplete catalogs.

`sync_legacy_symbols.dart` regenerates the checked-in Dart API. Placeholder
arguments in generated methods follow Flutter's generated (alphabetical) order.
