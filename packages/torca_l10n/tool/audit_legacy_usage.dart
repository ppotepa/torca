import 'dart:convert';
import 'dart:io';

void main() {
  final source = File(
    'lib/src/legacy_localizations.dart',
  ).readAsStringSync();
  final catalog = jsonDecode(
    File('lib/l10n/torca_en.arb').readAsStringSync(),
  ) as Map<String, dynamic>;
  final keys = catalog.keys.where((key) => !key.startsWith('@')).toSet();
  final legacy = <String>{};
  for (final match in RegExp(
    r'^\s*String\s+(?:get\s+)?([A-Za-z][A-Za-z0-9_]*)\s*(?:\([^)]*\))?\s*[=({]',
    multiLine: true,
  ).allMatches(source)) {
    legacy.add(match.group(1)!);
  }
  final missing = legacy.difference(keys).toList()..sort();
  stdout.writeln('Legacy symbols: ${legacy.length}');
  stdout.writeln('Catalog keys: ${keys.length}');
  stdout.writeln('Missing from shared catalog: ${missing.length}');
  for (final key in missing) stdout.writeln('  $key');
  if (missing.isNotEmpty) exitCode = 2;
}
