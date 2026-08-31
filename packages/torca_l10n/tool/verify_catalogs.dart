import 'dart:convert';
import 'dart:io';

void main() {
  final directory = Directory('lib/l10n');
  final files =
      directory
          .listSync()
          .whereType<File>()
          .where((file) => file.path.endsWith('.arb'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  if (files.isEmpty) {
    stderr.writeln('No ARB catalogs found in ${directory.path}');
    exitCode = 1;
    return;
  }

  Map<String, dynamic> read(File file) =>
      jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;

  final catalogs = <String, Map<String, dynamic>>{};
  for (final file in files) {
    final catalog = read(file);
    final locale = catalog['@@locale'];
    final name = file.uri.pathSegments.last;
    final expected = name.substring('torca_'.length, name.length - 4);
    if (locale != expected) {
      throw FormatException(
        '$name declares @@locale=$locale, expected $expected',
      );
    }
    catalogs[expected] = catalog;
  }

  final template = catalogs['en'];
  if (template == null)
    throw const FormatException('English template is missing');
  final templateKeys = template.keys
      .where((key) => !key.startsWith('@'))
      .toSet();
  final templatePlaceholders = _placeholders(template);

  for (final entry in catalogs.entries) {
    final keys = entry.value.keys.where((key) => !key.startsWith('@')).toSet();
    final missing = templateKeys.difference(keys);
    final extra = keys.difference(templateKeys);
    if (missing.isNotEmpty || extra.isNotEmpty) {
      throw FormatException(
        '${entry.key}: keys differ; missing=$missing extra=$extra',
      );
    }
    final placeholders = _placeholders(entry.value);
    if (placeholders.length != templatePlaceholders.length ||
        !placeholders.entries.every(
          (entry) => templatePlaceholders[entry.key] == entry.value,
        )) {
      throw FormatException(
        '${entry.key}: placeholder definitions differ from English',
      );
    }
    for (final key in templateKeys) {
      final value = entry.value[key];
      if (value is! String || value.trim().isEmpty) {
        throw FormatException('${entry.key}: $key is empty or not a string');
      }
    }
  }
  stdout.writeln(
    'Verified ${catalogs.length} catalogs and ${templateKeys.length} keys.',
  );
}

Map<String, String> _placeholders(Map<String, dynamic> catalog) {
  final result = <String, String>{};
  for (final entry in catalog.entries) {
    if (!entry.key.startsWith('@') || entry.key == '@@locale') continue;
    final value = entry.value;
    if (value is! Map<String, dynamic>) continue;
    final placeholders = value['placeholders'];
    if (placeholders is! Map<String, dynamic>) continue;
    for (final placeholder in placeholders.entries) {
      final definition = placeholder.value;
      if (definition is Map<String, dynamic>) {
        result['${entry.key.substring(1)}.${placeholder.key}'] =
            '${definition['type'] ?? 'String'}';
      }
    }
  }
  return result;
}
