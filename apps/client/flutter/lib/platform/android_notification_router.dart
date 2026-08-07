import 'dart:io';

import 'package:flutter/services.dart';

import '../navigation/app_navigation_controller.dart';

class AndroidNotificationRouter {
  AndroidNotificationRouter(this.navigation);

  static const MethodChannel _channel = MethodChannel('torca/notifications');
  final AppNavigationController navigation;

  Future<void> initialize() async {
    if (!Platform.isAndroid) return;
    _channel.setMethodCallHandler((call) async {
      if (call.method != 'openConversation') return;
      final conversationId = call.arguments as String?;
      if (conversationId != null && conversationId.isNotEmpty) {
        navigation.openConversation(conversationId);
      }
    });
    final initial = await _channel.invokeMethod<String>('takeInitialConversation');
    if (initial != null && initial.isNotEmpty) {
      navigation.openConversation(initial);
    }
  }

  void dispose() {
    if (Platform.isAndroid) _channel.setMethodCallHandler(null);
  }
}
