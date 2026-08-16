import 'dart:io';

import 'package:flutter/services.dart';

/// Owns the single platform boundary used by Radio before enabling capture.
/// The Rust audio adapter still treats permission/device failures as
/// authoritative; this service exists to provide the native Android prompt
/// at the point where the user explicitly enables or presses PTT.
abstract final class MicrophonePermission {
  static const MethodChannel _channel = MethodChannel('torca/audio');

  static Future<bool> ensureGranted() async {
    if (!Platform.isAndroid) return true;
    try {
      if (await _channel.invokeMethod<bool>('hasMicrophonePermission') ==
          true) {
        return true;
      }
      return await _channel.invokeMethod<bool>('requestMicrophonePermission') ==
          true;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  /// Places Android in the voice-communication audio mode while a radio burst
  /// is active. This lets the platform route and voice DSP (when available)
  /// treat the microphone/render pair as a communications stream.
  static Future<void> setCommunicationMode(bool enabled) async {
    if (!Platform.isAndroid) return;
    try {
      await _channel.invokeMethod<void>(
        'setCommunicationAudioMode',
        <String, Object?>{'enabled': enabled},
      );
    } on MissingPluginException {
      // The Rust/CPAL fallback remains usable on hosts without the channel.
    } on PlatformException {
      // Audio mode is an enhancement; capture lifecycle remains authoritative.
    }
  }
}
