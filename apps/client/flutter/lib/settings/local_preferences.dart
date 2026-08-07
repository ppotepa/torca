import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

class LocalPreferences extends ChangeNotifier {
  LocalPreferences({SharedPreferencesAsync? store}) : _store = store ?? SharedPreferencesAsync();
  final SharedPreferencesAsync _store;
  static const _notificationsKey = 'notifications.enabled';
  static const _previewKey = 'notifications.message_preview';
  bool _notificationsEnabled = true;
  bool _messagePreview = false;
  bool get notificationsEnabled => _notificationsEnabled;
  bool get messagePreview => _messagePreview;

  Future<void> load() async {
    _notificationsEnabled = await _store.getBool(_notificationsKey) ?? true;
    _messagePreview = await _store.getBool(_previewKey) ?? false;
    await _cleanupOldTempExports();
    notifyListeners();
  }

  Future<void> setNotificationsEnabled(bool value) async {
    _notificationsEnabled = value;
    notifyListeners();
    await _store.setBool(_notificationsKey, value);
  }

  Future<void> setMessagePreview(bool value) async {
    _messagePreview = value;
    notifyListeners();
    await _store.setBool(_previewKey, value);
  }

  Future<void> _cleanupOldTempExports() async {
    final directory = Directory.systemTemp;
    final cutoff = DateTime.now().subtract(const Duration(hours: 24));
    try {
      await for (final entity in directory.list(followLinks: false)) {
        if (entity is! File) continue;
        final name = entity.uri.pathSegments.isEmpty ? '' : entity.uri.pathSegments.last;
        if (!RegExp(r'^torca-[0-9a-fA-F]{32}(\.[A-Za-z0-9]{1,10})?$').hasMatch(name)) continue;
        try {
          final stat = await entity.stat();
          if (stat.modified.isBefore(cutoff)) await entity.delete();
        } on FileSystemException {
          // Best-effort cleanup: an external viewer may still hold the export open.
        }
      }
    } on FileSystemException {
      // System temp may be unavailable under a restricted platform sandbox.
    }
  }
}
