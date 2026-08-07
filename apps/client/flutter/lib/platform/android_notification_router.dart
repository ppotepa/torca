import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';

import '../navigation/app_navigation_controller.dart';
import '../settings/local_preferences.dart';

class AndroidNotificationRouter {
  AndroidNotificationRouter(this.navigation, this.preferences);

  static const MethodChannel _channel = MethodChannel('torca/notifications');
  final AppNavigationController navigation;
  final LocalPreferences preferences;

  Future<void> initialize() async {
    if (!Platform.isAndroid) return;
    _channel.setMethodCallHandler((call) async {
      if (call.method != 'openConversation') return;
      final conversationId = call.arguments as String?;
      if (conversationId != null && conversationId.isNotEmpty) {
        navigation.openConversation(conversationId);
      }
    });
    preferences.addListener(_preferencesChanged);
    await _syncPreferences();
    final initial = await _channel.invokeMethod<String>('takeInitialConversation');
    if (initial != null && initial.isNotEmpty) {
      navigation.openConversation(initial);
    }
  }

  void _preferencesChanged() {
    unawaited(_syncPreferences());
  }

  Future<void> _syncPreferences() async {
    if (!Platform.isAndroid) return;
    await _channel.invokeMethod<void>(
      'setNotificationsEnabled',
      preferences.notificationsEnabled,
    );
  }

  void dispose() {
    if (!Platform.isAndroid) return;
    preferences.removeListener(_preferencesChanged);
    _channel.setMethodCallHandler(null);
  }
}
