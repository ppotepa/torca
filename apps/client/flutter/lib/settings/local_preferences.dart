import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/app_locale_mode.dart';
import '../theme/app_theme_mode.dart';

class LocalPreferences extends ChangeNotifier {
  LocalPreferences({SharedPreferencesAsync? store})
    : _store = _makeStore(store);

  final _PreferencesStore _store;

  static const _themeModeKey = 'appearance.theme_mode';
  static const _themeFamilyKey = 'appearance.theme_family';
  static const _themeVariantKey = 'appearance.theme_variant';
  static const _themeDensityKey = 'appearance.theme_density';
  static const _reduceMotionKey = 'appearance.reduce_motion';
  static const _localeModeKey = 'appearance.locale_mode';
  static const _closeToTrayKey = 'desktop.close_to_tray';

  AppThemeMode _themeMode = AppThemeMode.system;
  TorcaAppearance _appearance = const TorcaAppearance();
  // English is the safe pre-load presentation; load() applies the persisted/system choice.
  AppLocaleMode _localeMode = AppLocaleMode.english;
  bool _notificationsEnabled = true;
  bool _readReceiptsEnabled = true;
  bool _closeToTrayEnabled = true;
  Future<void> Function(bool enabled)? _runtimeNotificationSetter;
  Future<void> Function(bool enabled)? _runtimeReadReceiptSetter;

  AppThemeMode get themeMode => _themeMode;
  TorcaAppearance get appearance => _appearance;
  AppLocaleMode get localeMode => _localeMode;
  bool get notificationsEnabled => _notificationsEnabled;
  bool get readReceiptsEnabled => _readReceiptsEnabled;
  bool get closeToTrayEnabled => _closeToTrayEnabled;

  void attachRuntimeNotificationSetting(
    Future<void> Function(bool enabled) setter,
  ) {
    _runtimeNotificationSetter = setter;
  }

  void attachRuntimeReadReceiptSetting(
    Future<void> Function(bool enabled) setter,
  ) {
    _runtimeReadReceiptSetter = setter;
  }

  /// Mirrors the process-runtime setting for presentation and host notification rendering.
  /// Rust/SQLite remains the source of truth; this value is never persisted in Flutter.
  void syncNotificationsEnabled(bool value) {
    if (_notificationsEnabled == value) return;
    _notificationsEnabled = value;
    notifyListeners();
  }

  /// Mirrors the encrypted runtime privacy setting. Flutter does not persist a
  /// second copy, avoiding disagreement between the switch and delivery policy.
  void syncReadReceiptsEnabled(bool value) {
    if (_readReceiptsEnabled == value) return;
    _readReceiptsEnabled = value;
    notifyListeners();
  }

  Future<void> load() async {
    _themeMode = AppThemeMode.parse(await _store.getString(_themeModeKey));
    final family = TorcaAppearance.parseFamily(
      await _store.getString(_themeFamilyKey),
    );
    var variant = TorcaThemeVariant.parse(
      await _store.getString(_themeVariantKey),
    );
    if (variant.family != family) {
      variant = TorcaThemeVariant.values.firstWhere(
        (value) => value.family == family,
      );
    }
    _appearance = TorcaAppearance(
      family: family,
      variant: variant,
      density: TorcaAppearance.parseDensity(
        await _store.getString(_themeDensityKey),
      ),
      reduceMotion: await _store.getBool(_reduceMotionKey) ?? false,
    );
    _localeMode = parseAppLocaleMode(await _store.getString(_localeModeKey));
    _closeToTrayEnabled = await _store.getBool(_closeToTrayKey) ?? true;
    notifyListeners();
  }

  Future<void> setThemeMode(AppThemeMode value) async {
    if (_themeMode == value) return;
    _themeMode = value;
    notifyListeners();
    await _store.setString(_themeModeKey, value.storageValue);
  }

  Future<void> setThemeFamily(TorcaThemeFamily value) async {
    final next = _appearance.copyWith(family: value);
    if (_appearance == next) return;
    _appearance = next;
    notifyListeners();
    await _store.setString(_themeFamilyKey, value.name);
    await _store.setString(_themeVariantKey, next.variant.name);
  }

  Future<void> setThemeVariant(TorcaThemeVariant value) async {
    if (_appearance.variant == value) return;
    _appearance = _appearance.copyWith(family: value.family, variant: value);
    notifyListeners();
    await _store.setString(_themeFamilyKey, value.family.name);
    await _store.setString(_themeVariantKey, value.name);
  }

  Future<void> setThemeDensity(TorcaDensity value) async {
    if (_appearance.density == value) return;
    _appearance = _appearance.copyWith(density: value);
    notifyListeners();
    await _store.setString(_themeDensityKey, value.name);
  }

  Future<void> setReduceMotion(bool value) async {
    if (_appearance.reduceMotion == value) return;
    _appearance = _appearance.copyWith(reduceMotion: value);
    notifyListeners();
    await _store.setBool(_reduceMotionKey, value);
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
    await _runtimeNotificationSetter?.call(value);
  }

  Future<void> setReadReceiptsEnabled(bool value) async {
    if (_readReceiptsEnabled == value) return;
    _readReceiptsEnabled = value;
    notifyListeners();
    await _runtimeReadReceiptSetter?.call(value);
  }

  Future<void> setCloseToTrayEnabled(bool value) async {
    if (_closeToTrayEnabled == value) return;
    _closeToTrayEnabled = value;
    notifyListeners();
    await _store.setBool(_closeToTrayKey, value);
  }
}

_PreferencesStore _makeStore(SharedPreferencesAsync? store) {
  if (store != null) return _PlatformPreferencesStore(store);
  try {
    return _PlatformPreferencesStore(SharedPreferencesAsync());
  } on StateError {
    return _MemoryPreferencesStore();
  }
}

abstract interface class _PreferencesStore {
  Future<String?> getString(String key);
  Future<bool?> getBool(String key);
  Future<void> setString(String key, String value);
  Future<void> setBool(String key, bool value);
}

class _PlatformPreferencesStore implements _PreferencesStore {
  _PlatformPreferencesStore(this._store);
  final SharedPreferencesAsync _store;

  @override
  Future<String?> getString(String key) => _store.getString(key);
  @override
  Future<bool?> getBool(String key) => _store.getBool(key);
  @override
  Future<void> setString(String key, String value) =>
      _store.setString(key, value);
  @override
  Future<void> setBool(String key, bool value) => _store.setBool(key, value);
}

class _MemoryPreferencesStore implements _PreferencesStore {
  final Map<String, Object> _values = <String, Object>{
    'appearance.locale_mode': 'en',
  };

  @override
  Future<String?> getString(String key) async => _values[key] as String?;
  @override
  Future<bool?> getBool(String key) async => _values[key] as bool?;
  @override
  Future<void> setString(String key, String value) async =>
      _values[key] = value;
  @override
  Future<void> setBool(String key, bool value) async => _values[key] = value;
}
