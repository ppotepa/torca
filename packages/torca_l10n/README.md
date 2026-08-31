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
flutter gen-l10n
flutter test
```

The application may depend on this package for the locale registry and
generated delegate. During migration, the legacy `TorcaStrings` adapter is
kept in the app until every feature has moved to `TorcaLocalizations`.

`audit_legacy_usage.dart` is intentionally allowed to report remaining legacy
symbols during the migration. The deploy validation fails only on incomplete
shared catalogs; the audit output is the checklist for the next migration
module.
