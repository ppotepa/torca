import 'dart:convert';
import 'dart:io';

void main() {
  final files = Directory('lib/l10n')
      .listSync()
      .whereType<File>()
      .where((file) => file.path.endsWith('.arb'))
      .toList()
    ..sort((a, b) => a.path.compareTo(b.path));
  if (files.isEmpty) {
    stderr.writeln('No ARB catalogs found.');
    exitCode = 2;
    return;
  }
  final catalogs = files.map(_read).toList();
  final sourceKeys = catalogs.first.keys.where((key) => !key.startsWith('@')).toSet();
  final missing = <String>[];
  for (var index = 1; index < catalogs.length; index++) {
    final keys = catalogs[index].keys.where((key) => !key.startsWith('@')).toSet();
    for (final key in sourceKeys.difference(keys)) {
      missing.add('${files[index].path}: $key');
    }
    for (final key in keys.difference(sourceKeys)) {
      missing.add('${files.first.path}: $key');
    }
  }
  stdout.writeln('Shared catalog keys: ${sourceKeys.length}');
  stdout.writeln('Catalogs: ${files.length}');
  stdout.writeln('Key mismatches: ${missing.length}');
  for (final item in missing) stdout.writeln('  $item');
  if (missing.isNotEmpty) exitCode = 2;
}

Map<String, dynamic> _read(File file) =>
    jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;
