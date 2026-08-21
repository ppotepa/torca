import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/localization/app_locale_mode.dart';
import 'package:torca_app/settings/battery_preferences.dart';
import 'package:torca_app/settings/local_preferences.dart';
import 'package:torca_app/theme/app_theme_mode.dart';

void main() {
  test('non-shell preferences do not invalidate the app shell', () async {
    final preferences = LocalPreferences();
    var shellChanges = 0;
    preferences.shellChanges.addListener(() => shellChanges++);

    await preferences.setBatteryMode(TorcaBatteryMode.automatic);
    expect(shellChanges, 0);

    await preferences.setThemeMode(AppThemeMode.dark);
    expect(shellChanges, 1);

    await preferences.setLocaleMode(AppLocaleMode.polish);
    expect(shellChanges, 2);

    preferences.dispose();
  });
}
