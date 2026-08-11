import 'dart:io';

import 'package:flutter/services.dart';

import '../navigation/app_navigation_controller.dart';

class AndroidNotificationRouter {
  AndroidNotificationRouter(this.navigation);

  static const MethodChannel _channel = MethodChannel('torca/notifications');
  final AppNavigationController navigation;
  bool _disposed = false;

  Future<void> initialize() async {
    if (!Platform.isAndroid || _disposed) return;
    _channel.setMethodCallHandler((call) async {
      if (_disposed) return;
      if (call.method != 'openConversation') return;
      final conversationId = call.arguments as String?;
      if (conversationId != null && conversationId.isNotEmpty) {
        navigation.openConversation(conversationId);
      }
    });
    final initial = await _channel.invokeMethod<String>(
      'takeInitialConversation',
    );
    if (!_disposed && initial != null && initial.isNotEmpty) {
      navigation.openConversation(initial);
    }
  }

  void dispose() {
    if (!Platform.isAndroid || _disposed) return;
    _disposed = true;
    _channel.setMethodCallHandler(null);
  }
}
