import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

abstract final class AvatarDeviceSeed {
  static const MethodChannel _channel = MethodChannel('torca/device');
  static Future<String>? _resolved;
  static String? _platformIdentifierOverride;

  @visibleForTesting
  static void overridePlatformIdentifier(String? value) {
    _platformIdentifierOverride = value;
    _resolved = null;
  }

  /// Returns a pseudonymous, application-scoped seed stable across reinstall.
  /// The raw platform identifier never leaves this process and is never put in
  /// pairing/contact payloads; only the generated avatar genome is exchanged.
  static Future<String> resolve({String? fallbackIdentity}) {
    final cached = _resolved;
    if (cached != null) return cached;
    final future = _resolve(fallbackIdentity);
    _resolved = future;
    // Do not permanently poison this cache after a transient platform or I/O
    // failure. A later profile retry must be able to resolve the identifier
    // again after Android has finished attaching its channels.
    future.then<void>(
      (_) {},
      onError: (Object _, StackTrace __) {
        if (identical(_resolved, future)) _resolved = null;
      },
    );
    return future;
  }

  static Future<String> _resolve(String? fallbackIdentity) async {
    String? raw = _platformIdentifierOverride;
    if (raw != null) {
      // Tests use a deterministic identifier without spawning platform
      // processes or invoking method channels from FakeAsync.
    } else if (Platform.isAndroid) {
      try {
        raw = await _channel.invokeMethod<String>('stableDeviceId');
      } on MissingPluginException {
        raw = null;
      } on PlatformException {
        raw = null;
      }
    } else if (Platform.isWindows) {
      raw = await _windowsMachineIdentifier();
    }
    raw = raw?.trim();
    if (raw == null || raw.isEmpty) {
      raw = fallbackIdentity?.trim();
    }
    if (raw == null || raw.isEmpty) {
      raw = '${Platform.operatingSystem}:${Platform.localHostname}';
    }
    return fromPlatformIdentifier(raw);
  }

  static String fromPlatformIdentifier(String value) => sha256
      .convert(utf8.encode('torca-avatar-device-v1:${value.trim()}'))
      .toString();

  static Future<String?> _windowsMachineIdentifier() async {
    try {
      final result = await Process.run('reg.exe', const <String>[
        'query',
        r'HKLM\SOFTWARE\Microsoft\Cryptography',
        '/v',
        'MachineGuid',
      ], runInShell: false);
      if (result.exitCode == 0) {
        final match = RegExp(
          r'MachineGuid\s+REG_SZ\s+([^\r\n]+)',
          caseSensitive: false,
        ).firstMatch(result.stdout.toString());
        final machineGuid = match?.group(1)?.trim();
        if (machineGuid != null && machineGuid.isNotEmpty) return machineGuid;
      }
    } on Object {
      // Restricted Windows environments fall back to the stable hostname.
    }
    return Platform.environment['COMPUTERNAME'];
  }
}
