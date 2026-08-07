import 'dart:ffi' as ffi;
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';
import 'engine_gateway.dart';

typedef _MaxAttachmentBytesNative = ffi.Uint64 Function();
typedef _MaxAttachmentBytesDart = int Function();

/// Adds native read-only capability metadata without moving domain behavior
/// out of the canonical engine gateway.
class NativeCapabilitiesGateway
    implements EngineGateway, AttachmentCapabilitiesProvider {
  NativeCapabilitiesGateway(this._delegate, {ffi.DynamicLibrary? library})
      : _library = library ?? ffi.DynamicLibrary.open(_libraryName()) {
    _maxAttachmentBytes = _library
        .lookupFunction<_MaxAttachmentBytesNative, _MaxAttachmentBytesDart>(
          'torca_max_attachment_bytes',
        )();
  }

  final EngineGateway _delegate;
  final ffi.DynamicLibrary _library;
  late final int _maxAttachmentBytes;

  @override
  AppCapabilities get capabilities =>
      AppCapabilities(maxAttachmentBytes: _maxAttachmentBytes);

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _delegate.snapshots;

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) =>
      _delegate.execute(command);

  @override
  Future<String> diagnosticsJson() => _delegate.diagnosticsJson();

  @override
  Future<void> dispose() => _delegate.dispose();

  static String _libraryName() {
    if (Platform.isWindows) return 'torca_bridge.dll';
    if (Platform.isAndroid || Platform.isLinux) return 'libtorca_bridge.so';
    if (Platform.isMacOS || Platform.isIOS) return 'libtorca_bridge.dylib';
    throw UnsupportedError('Torca native runtime is unsupported on this platform');
  }
}
