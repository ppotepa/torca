import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/widgets.dart';
import 'package:torca_l10n/torca_l10n.dart';

void main() {
  test('registry exposes one definition per generated catalog', () {
    expect(
      TorcaLocaleRegistry.locales.map((locale) => locale.languageCode),
      containsAll(<String>['en', 'pl', 'de', 'es', 'fr', 'uk']),
    );
    expect(TorcaLocaleRegistry.definitions, hasLength(6));
  });

  test('locale metadata is stable and discoverable by language code', () {
    expect(TorcaLocaleRegistry.find(const Locale('de'))?.nativeName, 'Deutsch');
    expect(TorcaLocaleRegistry.find(const Locale('uk'))?.flag, '🇺🇦');
    expect(
      TorcaLocaleRegistry.find(const Locale('de', 'AT'))?.locale,
      const Locale('de'),
    );
    expect(TorcaLocaleRegistry.find(const Locale('xx')), isNull);
  });
}
