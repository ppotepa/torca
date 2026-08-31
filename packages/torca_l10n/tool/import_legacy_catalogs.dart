import 'dart:convert';
import 'dart:io';

/// Imports the current application's legacy ARB keys into the shared package.
///
/// This is intentionally a one-way migration helper. New keys must be added
/// to torca_en.arb first; the verification gate remains the source of truth
/// after this import.
void main() {
  final packageDirectory = Directory('lib/l10n');
  final appDirectory = Directory('../../apps/client/flutter/lib/l10n');
  final english = _read(File('${appDirectory.path}/app_en.arb'));
  final polish = _read(File('${appDirectory.path}/app_pl.arb'));
  final current = <String, Map<String, dynamic>>{};
  for (final locale in const <String>['en', 'pl', 'de', 'es', 'fr', 'uk']) {
    current[locale] = _read(File('${packageDirectory.path}/torca_$locale.arb'));
  }

  for (final locale in current.keys) {
    final catalog = <String, dynamic>{
      ...english,
      ...current[locale]!,
      if (locale == 'pl') ...polish,
    };
    catalog['@@locale'] = locale;
    final output = File('${packageDirectory.path}/torca_$locale.arb');
    output.writeAsStringSync(
      '${const JsonEncoder.withIndent('  ').convert(catalog)}\n',
    );
  }
  stdout.writeln('Imported legacy keys into ${current.length} catalogs.');
}

Map<String, dynamic> _read(File file) =>
    jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;
