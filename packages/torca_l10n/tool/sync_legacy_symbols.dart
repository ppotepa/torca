import 'dart:convert';
import 'dart:io';

/// Keeps the shared catalog API complete while the application is migrated
/// feature by feature from the old TorcaStrings adapter.
///
/// This is intentionally one-way: it adds missing symbols and never rewrites
/// existing translations. New translations can therefore be edited directly
/// in the ARB files after the migration pass.
void main() {
  final legacy = File(
    '${Directory.current.path}/lib/src/legacy_localizations.dart',
  ).readAsStringSync();
  final arbDir = Directory('${Directory.current.path}/lib/l10n');
  final files = arbDir
      .listSync()
      .whereType<File>()
      .where((file) => file.path.endsWith('.arb'))
      .toList()
    ..sort((a, b) => a.path.compareTo(b.path));

  final symbols = <String, List<String>>{};
  final declaration = RegExp(
    r'^\s*String\s+(?:get\s+)?([a-zA-Z]\w*)\s*(?:\(([^)]*)\))?\s*(?:=>|\{)',
    multiLine: true,
  );
  for (final match in declaration.allMatches(legacy)) {
    final name = match.group(1)!;
    if (name.startsWith('_')) continue;
    final params = _parameterNames(match.group(2));
    symbols[name] = params;
  }

  final english = _read(files.firstWhere((file) => file.path.endsWith('torca_en.arb')));
  final polish = _read(files.firstWhere((file) => file.path.endsWith('torca_pl.arb')));
  var added = 0;
  for (final entry in symbols.entries) {
    if (english.containsKey(entry.key)) continue;
    final value = entry.value.isEmpty
        ? _humanize(entry.key)
        : entry.value.map((name) => '{$name}').join(' ');
    english[entry.key] = value;
    polish[entry.key] = value;
    for (final file in files) {
      if (file.path.endsWith('torca_en.arb') || file.path.endsWith('torca_pl.arb')) {
        continue;
      }
      final catalog = _read(file);
      catalog[entry.key] = value;
      _write(file, catalog);
    }
    added++;
  }
  _write(files.firstWhere((file) => file.path.endsWith('torca_en.arb')), english);
  _write(files.firstWhere((file) => file.path.endsWith('torca_pl.arb')), polish);
  stdout.writeln('Added $added legacy symbols to shared catalogs.');
}

List<String> _parameterNames(String? signature) {
  if (signature == null || signature.trim().isEmpty) return const [];
  return signature
      .split(',')
      .map((part) => part.trim().replaceFirst(RegExp(r'^\{'), '').replaceFirst(RegExp(r'\}$'), ''))
      .map((part) => part.split(RegExp(r'\s+')).last.replaceAll('?', ''))
      .where((name) => RegExp(r'^[a-zA-Z]\w*$').hasMatch(name))
      .toList();
}

Map<String, dynamic> _read(File file) =>
    jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;

void _write(File file, Map<String, dynamic> catalog) {
  final locale = catalog['@@locale'];
  final ordered = <String, dynamic>{'@@locale': locale};
  final keys = catalog.keys.where((key) => key != '@@locale').toList()..sort();
  for (final key in keys) {
    ordered[key] = catalog[key];
  }
  file.writeAsStringSync('${const JsonEncoder.withIndent('  ').convert(ordered)}\n');
}

String _humanize(String value) => value
    .replaceAllMapped(RegExp(r'([a-z])([A-Z])'), (match) => '${match.group(1)} ${match.group(2)}')
    .replaceFirst(value[0], value[0].toUpperCase());
