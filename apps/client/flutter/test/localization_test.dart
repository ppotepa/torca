import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/localization/app_locale_mode.dart';
import 'package:torca_app/localization/torca_strings.dart';

void main() {
  test('locale preference parsing is stable', () {
    expect(parseAppLocaleMode(null), AppLocaleMode.system);
    expect(parseAppLocaleMode('en'), AppLocaleMode.english);
    expect(parseAppLocaleMode('pl'), AppLocaleMode.polish);
    expect(parseAppLocaleMode('de'), AppLocaleMode.german);
    expect(parseAppLocaleMode('es'), AppLocaleMode.spanish);
    expect(parseAppLocaleMode('fr'), AppLocaleMode.french);
    expect(parseAppLocaleMode('uk'), AppLocaleMode.ukrainian);
  });

  test('Polish catalog exposes translated primary settings strings', () {
    const strings = TorcaStrings(Locale('pl'));
    expect(strings.settings, 'Ustawienia');
    expect(strings.enableNotifications, 'Włącz powiadomienia');
    expect(strings.closeToTray, 'Zamykaj do zasobnika');
  });

  test('new locale catalogs translate onboarding and settings', () {
    const expected = <String, (String, String, String)>{
      'de': ('Einstellungen', 'Sprache', 'Wähle deinen Spitznamen'),
      'es': ('Ajustes', 'Idioma', 'Elige tu apodo'),
      'fr': ('Paramètres', 'Langue', 'Choisissez votre pseudonyme'),
      'uk': ('Налаштування', 'Мова', 'Оберіть псевдонім'),
    };
    for (final entry in expected.entries) {
      final strings = TorcaStrings(Locale(entry.key));
      expect(strings.settings, entry.value.$1);
      expect(strings.language, entry.value.$2);
      expect(strings.chooseNickname, entry.value.$3);
    }
  });
}
