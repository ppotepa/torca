import 'dart:io';

import 'package:flutter/services.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../navigation/app_navigation_controller.dart';

class AndroidNotificationRouter {
  AndroidNotificationRouter(this.navigation, this.gateway);

  static const MethodChannel _channel = MethodChannel('torca/notifications');
  final AppNavigationController navigation;
  final EngineGateway gateway;
  bool _disposed = false;

  Future<void> initialize() async {
    if (!Platform.isAndroid || _disposed) return;
    _channel.setMethodCallHandler((call) async {
      if (_disposed) return;
      if (call.method == 'openConversation') {
        _open(call.arguments as String?);
      } else if (call.method == 'notificationAction') {
        await _handleAction(call.arguments);
      }
    });
    final initialAction = await _channel.invokeMethod<dynamic>(
      'takeInitialNotificationAction',
    );
    if (!_disposed && initialAction is Map) {
      await _handleAction(initialAction);
      return;
    }
    final initial = await _channel.invokeMethod<String>(
      'takeInitialConversation',
    );
    _open(initial);
  }

  void _open(String? conversationId) {
    if (_disposed || conversationId == null || conversationId.isEmpty) return;
    navigation.openConversation(conversationId);
  }

  Future<void> _handleAction(Object? raw) async {
    if (_disposed || raw is! Map) return;
    final conversationId = raw['conversationId'] as String?;
    final pairingId = raw['pairingId'] as String?;
    final action = raw['action'] as String? ?? 'open';
    final replyText = raw['replyText'] as String?;
    if (action == 'approve' && pairingId != null && pairingId.isNotEmpty) {
      await gateway.execute(ApprovePairingCommandDto(sessionIdHex: pairingId));
      navigation.openPairingSession(pairingId);
      return;
    }
    if (action == 'reject' && pairingId != null && pairingId.isNotEmpty) {
      await gateway.execute(RejectPairingCommandDto(sessionIdHex: pairingId));
      return;
    }
    if (action == 'reply' &&
        conversationId != null &&
        conversationId.isNotEmpty &&
        replyText != null &&
        replyText.trim().isNotEmpty) {
      await gateway.execute(
        QueueMessageCommandDto(
          conversationIdHex: conversationId,
          body: replyText.trim(),
        ),
      );
      _open(conversationId);
      return;
    }
    if (conversationId == null || conversationId.isEmpty) {
      if (pairingId != null && pairingId.isNotEmpty) {
        navigation.openPairingSession(pairingId);
      }
      return;
    }
    if (action == 'mark_read') {
      await gateway.execute(
        MarkConversationReadCommandDto(conversationIdHex: conversationId),
      );
    }
    _open(conversationId);
  }

  void dispose() {
    if (!Platform.isAndroid || _disposed) return;
    _disposed = true;
    _channel.setMethodCallHandler(null);
  }
}
