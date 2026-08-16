import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/app_locale_mode.dart';
import '../theme/app_theme_mode.dart';
import 'battery_preferences.dart';

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
  static const _audioInputDeviceKey = 'desktop.audio_input_device';
  static const _audioOutputDeviceKey = 'desktop.audio_output_device';
  static const _batteryModeKey = 'battery.mode';
  static const _backgroundSyncKey = 'battery.background_sync';
  static const _delayedDeliveryKey = 'battery.allow_delayed_delivery';
  static const _meteredTransfersKey = 'battery.metered_transfers';
  static const _visualActivityKey = 'battery.visual_activity';

  static String _draftKey(String conversationId) =>
      'conversation.draft.$conversationId';
  static String _conversationPinnedKey(String conversationId) =>
      'conversation.pinned.$conversationId';
  static String _conversationMutedKey(String conversationId) =>
      'conversation.muted.$conversationId';
  static String _messageBookmarksKey(String conversationId) =>
      'conversation.bookmarks.$conversationId';

  AppThemeMode _themeMode = AppThemeMode.system;
  TorcaAppearance _appearance = const TorcaAppearance();
  // English is the safe pre-load presentation; load() applies the persisted/system choice.
  AppLocaleMode _localeMode = AppLocaleMode.english;
  bool _notificationsEnabled = true;
  bool _readReceiptsEnabled = true;
  bool _closeToTrayEnabled = true;
  String? _audioInputDeviceId;
  String? _audioOutputDeviceId;
  TorcaBatteryMode _batteryMode = TorcaBatteryMode.automatic;
  TorcaBackgroundSyncCadence _backgroundSync =
      TorcaBackgroundSyncCadence.instant;
  bool _allowDelayedBackgroundDelivery = false;
  TorcaMeteredTransferPolicy _meteredTransfers =
      TorcaMeteredTransferPolicy.pauseLarge;
  TorcaVisualActivityPolicy _visualActivity =
      TorcaVisualActivityPolicy.followSystem;
  // The app shell must not rebuild for unrelated preference changes (audio,
  // battery, privacy, etc.). This revision only changes for values consumed
  // by MaterialApp itself.
  final ValueNotifier<int> _shellRevision = ValueNotifier<int>(0);
  int _appearanceRevision = 0;
  Future<void> _appearanceWrite = Future<void>.value();
  Future<void> Function(bool enabled)? _runtimeNotificationSetter;
  Future<void> Function(bool enabled)? _runtimeReadReceiptSetter;
  Future<void> Function(String? inputId, String? outputId)? _runtimeAudioSetter;
  Future<void> Function(
    TorcaBatteryMode mode,
    TorcaBackgroundSyncCadence backgroundSync,
    bool allowDelayedBackgroundDelivery,
    TorcaMeteredTransferPolicy meteredTransfers,
    TorcaVisualActivityPolicy visualActivity,
  )?
  _runtimeBatterySetter;

  AppThemeMode get themeMode => _themeMode;
  TorcaAppearance get appearance => _appearance;
  AppLocaleMode get localeMode => _localeMode;
  bool get notificationsEnabled => _notificationsEnabled;
  bool get readReceiptsEnabled => _readReceiptsEnabled;
  bool get closeToTrayEnabled => _closeToTrayEnabled;
  String? get audioInputDeviceId => _audioInputDeviceId;
  String? get audioOutputDeviceId => _audioOutputDeviceId;
  TorcaBatteryMode get batteryMode => _batteryMode;
  TorcaBackgroundSyncCadence get backgroundSync => _backgroundSync;
  bool get allowDelayedBackgroundDelivery => _allowDelayedBackgroundDelivery;
  TorcaMeteredTransferPolicy get meteredTransfers => _meteredTransfers;
  TorcaVisualActivityPolicy get visualActivity => _visualActivity;
  Listenable get shellChanges => _shellRevision;

  void _notifyShellChanged() {
    _shellRevision.value++;
  }

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

  void attachRuntimeAudioSetting(
    Future<void> Function(String? inputId, String? outputId) setter,
  ) {
    _runtimeAudioSetter = setter;
  }

  void attachRuntimeBatterySetting(
    Future<void> Function(
      TorcaBatteryMode mode,
      TorcaBackgroundSyncCadence backgroundSync,
      bool allowDelayedBackgroundDelivery,
      TorcaMeteredTransferPolicy meteredTransfers,
      TorcaVisualActivityPolicy visualActivity,
    )
    setter,
  ) {
    _runtimeBatterySetter = setter;
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
    _audioInputDeviceId = _nonEmpty(
      await _store.getString(_audioInputDeviceKey),
    );
    _audioOutputDeviceId = _nonEmpty(
      await _store.getString(_audioOutputDeviceKey),
    );
    _batteryMode = TorcaBatteryMode.parse(
      await _store.getString(_batteryModeKey),
    );
    _backgroundSync = TorcaBackgroundSyncCadence.parse(
      await _store.getString(_backgroundSyncKey),
    );
    _allowDelayedBackgroundDelivery =
        await _store.getBool(_delayedDeliveryKey) ?? false;
    _meteredTransfers = TorcaMeteredTransferPolicy.parse(
      await _store.getString(_meteredTransfersKey),
    );
    _visualActivity = TorcaVisualActivityPolicy.parse(
      await _store.getString(_visualActivityKey),
    );
    notifyListeners();
    _notifyShellChanged();
  }

  Future<void> setThemeMode(AppThemeMode value) async {
    if (_themeMode == value) return;
    _themeMode = value;
    notifyListeners();
    _notifyShellChanged();
    _queueAppearanceWrite();
    await _appearanceWrite;
  }

  Future<void> setThemeFamily(TorcaThemeFamily value) async {
    final next = _appearance.copyWith(family: value);
    if (_appearance == next) return;
    _appearance = next;
    notifyListeners();
    _notifyShellChanged();
    _queueAppearanceWrite();
    await _appearanceWrite;
  }

  Future<void> setThemeVariant(TorcaThemeVariant value) async {
    if (_appearance.variant == value) return;
    _appearance = _appearance.copyWith(family: value.family, variant: value);
    notifyListeners();
    _notifyShellChanged();
    _queueAppearanceWrite();
    await _appearanceWrite;
  }

  Future<void> setThemeDensity(TorcaDensity value) async {
    if (_appearance.density == value) return;
    _appearance = _appearance.copyWith(density: value);
    notifyListeners();
    _notifyShellChanged();
    _queueAppearanceWrite();
    await _appearanceWrite;
  }

  Future<void> setReduceMotion(bool value) async {
    if (_appearance.reduceMotion == value) return;
    _appearance = _appearance.copyWith(reduceMotion: value);
    notifyListeners();
    _notifyShellChanged();
    _queueAppearanceWrite();
    await _appearanceWrite;
  }

  Future<void> setLocaleMode(AppLocaleMode value) async {
    if (_localeMode == value) return;
    _localeMode = value;
    notifyListeners();
    _notifyShellChanged();
    await _store.setString(_localeModeKey, value.storageValue);
  }

  void _queueAppearanceWrite() {
    final revision = ++_appearanceRevision;
    _appearanceWrite = _appearanceWrite
        .catchError((Object error, StackTrace stackTrace) {
          debugPrint('torca-preferences: persistence_error $error');
          debugPrintStack(stackTrace: stackTrace);
        })
        .then((_) async {
          // Coalesce rapid picker changes. Only the latest complete appearance
          // is persisted, preventing family/variant pairs from being torn or
          // stale.
          if (revision != _appearanceRevision) return;
          final appearance = _appearance;
          await _store.setString(_themeModeKey, _themeMode.storageValue);
          await _store.setString(_themeFamilyKey, appearance.family.name);
          await _store.setString(_themeVariantKey, appearance.variant.name);
          await _store.setString(_themeDensityKey, appearance.density.name);
          await _store.setBool(_reduceMotionKey, appearance.reduceMotion);
        });
  }

  @override
  void dispose() {
    _shellRevision.dispose();
    super.dispose();
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

  Future<void> setAudioInputDevice(String? value) async {
    if (_audioInputDeviceId == value) return;
    _audioInputDeviceId = value;
    notifyListeners();
    await _store.setString(_audioInputDeviceKey, value ?? '');
    await _runtimeAudioSetter?.call(_audioInputDeviceId, _audioOutputDeviceId);
  }

  Future<void> setAudioOutputDevice(String? value) async {
    if (_audioOutputDeviceId == value) return;
    _audioOutputDeviceId = value;
    notifyListeners();
    await _store.setString(_audioOutputDeviceKey, value ?? '');
    await _runtimeAudioSetter?.call(_audioInputDeviceId, _audioOutputDeviceId);
  }

  Future<void> setBatteryMode(TorcaBatteryMode value) async {
    if (_batteryMode == value) return;
    _batteryMode = value;
    notifyListeners();
    await _store.setString(_batteryModeKey, value.wireValue);
    await _pushBatteryPreferences();
  }

  Future<void> setBackgroundSync(TorcaBackgroundSyncCadence value) async {
    if (_backgroundSync == value) return;
    _backgroundSync = value;
    notifyListeners();
    await _store.setString(_backgroundSyncKey, value.wireValue);
    await _pushBatteryPreferences();
  }

  Future<void> setAllowDelayedBackgroundDelivery(bool value) async {
    if (_allowDelayedBackgroundDelivery == value) return;
    _allowDelayedBackgroundDelivery = value;
    notifyListeners();
    await _store.setBool(_delayedDeliveryKey, value);
    await _pushBatteryPreferences();
  }

  Future<void> setMeteredTransfers(TorcaMeteredTransferPolicy value) async {
    if (_meteredTransfers == value) return;
    _meteredTransfers = value;
    notifyListeners();
    await _store.setString(_meteredTransfersKey, value.wireValue);
    await _pushBatteryPreferences();
  }

  Future<void> setVisualActivity(TorcaVisualActivityPolicy value) async {
    if (_visualActivity == value) return;
    _visualActivity = value;
    notifyListeners();
    await _store.setString(_visualActivityKey, value.wireValue);
    await _pushBatteryPreferences();
  }

  Future<void> _pushBatteryPreferences() async {
    await _runtimeBatterySetter?.call(
      _batteryMode,
      _backgroundSync,
      _allowDelayedBackgroundDelivery,
      _meteredTransfers,
      _visualActivity,
    );
  }

  /// Drafts are local-only UI state and are never sent through the runtime
  /// contract. Empty values remove the persisted draft logically.
  Future<String?> draftFor(String conversationId) =>
      _store.getString(_draftKey(conversationId));

  Future<void> setDraft(String conversationId, String value) =>
      _store.setString(_draftKey(conversationId), value);

  Future<void> clearDraft(String conversationId) =>
      _store.setString(_draftKey(conversationId), '');

  Future<bool> conversationPinned(String conversationId) async =>
      await _store.getBool(_conversationPinnedKey(conversationId)) ?? false;

  Future<void> setConversationPinned(String conversationId, bool value) =>
      _store.setBool(_conversationPinnedKey(conversationId), value);

  Future<bool> conversationMuted(String conversationId) async =>
      await _store.getBool(_conversationMutedKey(conversationId)) ?? false;

  Future<void> setConversationMuted(String conversationId, bool value) =>
      _store.setBool(_conversationMutedKey(conversationId), value);

  Future<Set<String>> bookmarkedMessagesFor(String conversationId) async {
    final value = await _store.getString(_messageBookmarksKey(conversationId));
    if (value == null || value.trim().isEmpty) return <String>{};
    return value
        .split(',')
        .map((item) => item.trim())
        .where((item) => item.isNotEmpty)
        .toSet();
  }

  Future<void> setBookmarkedMessages(
    String conversationId,
    Set<String> messageIds,
  ) async {
    final sorted = messageIds.toList()..sort();
    await _store.setString(
      _messageBookmarksKey(conversationId),
      sorted.join(','),
    );
  }
}

String? _nonEmpty(String? value) =>
    value == null || value.isEmpty ? null : value;

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
