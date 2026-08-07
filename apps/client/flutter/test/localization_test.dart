import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/localization/app_locale_mode.dart';
import 'package:torca_app/localization/torca_strings.dart';

void main() {
  test('locale preference parsing is stable', () {
    expect(parseAppLocaleMode(null), AppLocaleMode.system);
    expect(parseAppLocaleMode('en'), AppLocaleMode.english);
    expect(parseAppLocaleMode('pl'), AppLocaleMode.polish);
  });

  test('Polish catalog exposes translated primary settings strings', () {
    const strings = TorcaStrings(Locale('pl'));
    expect(strings.settings, 'Ustawienia');
    expect(strings.enableNotifications, 'Włącz powiadomienia');
    expect(strings.closeToTray, 'Zamykaj do zasobnika');
  });
}
