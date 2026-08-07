import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../localization/app_locale_mode.dart';
import '../theme/app_theme_mode.dart';

class LocalPreferences extends ChangeNotifier {
  LocalPreferences({SharedPreferencesAsync? store})
      : _store = store ?? SharedPreferencesAsync();

  final SharedPreferencesAsync _store;

  static const _themeModeKey = 'appearance.theme_mode';
  static const _localeModeKey = 'appearance.locale_mode';
  static const _notificationsKey = 'notifications.enabled';
  static const _closeToTrayKey = 'desktop.close_to_tray';

  AppThemeMode _themeMode = AppThemeMode.system;
  AppLocaleMode _localeMode = AppLocaleMode.system;
  bool _notificationsEnabled = true;
  bool _closeToTrayEnabled = true;

  AppThemeMode get themeMode => _themeMode;
  AppLocaleMode get localeMode => _localeMode;
  bool get notificationsEnabled => _notificationsEnabled;
  bool get closeToTrayEnabled => _closeToTrayEnabled;

  Future<void> load() async {
    _themeMode = AppThemeMode.parse(await _store.getString(_themeModeKey));
    _localeMode = parseAppLocaleMode(await _store.getString(_localeModeKey));
    _notificationsEnabled = await _store.getBool(_notificationsKey) ?? true;
    _closeToTrayEnabled = await _store.getBool(_closeToTrayKey) ?? true;
    notifyListeners();
  }

  Future<void> setThemeMode(AppThemeMode value) async {
    if (_themeMode == value) return;
    _themeMode = value;
    notifyListeners();
    await _store.setString(_themeModeKey, value.storageValue);
  }

  Future<void> setLocaleMode(AppLocaleMode value) async {
    if (_localeMode == value) return;
    _localeMode = value;
    notifyListeners();
    await _store.setString(_localeModeKey, value.storageValue);
  }

  Future<void> setNotificationsEnabled(bool value) async {
    if (_notificationsEnabled == value) return;
    _notificationsEnabled = value;
    notifyListeners();
    await _store.setBool(_notificationsKey, value);
  }

  Future<void> setCloseToTrayEnabled(bool value) async {
    if (_closeToTrayEnabled == value) return;
    _closeToTrayEnabled = value;
    notifyListeners();
    await _store.setBool(_closeToTrayKey, value);
  }
}
