import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';
import 'engine_gateway.dart';

typedef _EngineHandle = ffi.Pointer<ffi.Void>;
typedef _EngineNewNative = _EngineHandle Function();
typedef _EngineNewDart = _EngineHandle Function();
typedef _EngineDestroyNative = ffi.Void Function(_EngineHandle);
typedef _EngineDestroyDart = void Function(_EngineHandle);
typedef _ContractVersionNative = ffi.Uint16 Function();
typedef _ContractVersionDart = int Function();
typedef _AllocNative = ffi.Pointer<ffi.Uint8> Function(ffi.UintPtr);
typedef _AllocDart = ffi.Pointer<ffi.Uint8> Function(int);
typedef _FreeNative = ffi.Void Function(ffi.Pointer<ffi.Uint8>, ffi.UintPtr);
typedef _FreeDart = void Function(ffi.Pointer<ffi.Uint8>, int);
typedef _CreateIdentityNative = ffi.Int32 Function(
  _EngineHandle,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Int64,
);
typedef _CreateIdentityDart = int Function(
  _EngineHandle,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
  int,
);
typedef _StartPairingNative = _CreateIdentityNative;
typedef _StartPairingDart = _CreateIdentityDart;
typedef _QueueMessageNative = ffi.Int32 Function(
  _EngineHandle,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Int64,
);
typedef _QueueMessageDart = int Function(
  _EngineHandle,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
  int,
);
typedef _RefreshSnapshotNative = ffi.Int32 Function(_EngineHandle);
typedef _RefreshSnapshotDart = int Function(_EngineHandle);
typedef _BufferPointerNative = ffi.Pointer<ffi.Uint8> Function(_EngineHandle);
typedef _BufferPointerDart = ffi.Pointer<ffi.Uint8> Function(_EngineHandle);
typedef _BufferLengthNative = ffi.UintPtr Function(_EngineHandle);
typedef _BufferLengthDart = int Function(_EngineHandle);
typedef _CloseNative = ffi.Int32 Function(_EngineHandle);
typedef _CloseDart = int Function(_EngineHandle);

/// Production gateway shared by every Flutter platform.
///
/// Platform runners only need to package the `torca_bridge` dynamic library. All command and
/// snapshot semantics remain in the Rust ClientEngine.
class FfiEngineGateway implements EngineGateway {
  FfiEngineGateway._(this._bindings, this._handle);

  static FfiEngineGateway open({ffi.DynamicLibrary? library}) {
    final _NativeBindings bindings = _NativeBindings(
      library ?? ffi.DynamicLibrary.open(_libraryName()),
    );
    if (bindings.contractVersion() != torcaContractVersion) {
      throw StateError(
        'native Torca contract ${bindings.contractVersion()} does not match '
        'Flutter contract $torcaContractVersion',
      );
    }
    final _EngineHandle handle = bindings.engineNew();
    if (handle == ffi.nullptr) {
      throw StateError('native Torca engine could not be created');
    }
    return FfiEngineGateway._(bindings, handle);
  }

  static String _libraryName() {
    if (Platform.isWindows) {
      return 'torca_bridge.dll';
    }
    if (Platform.isAndroid || Platform.isLinux) {
      return 'libtorca_bridge.so';
    }
    if (Platform.isMacOS || Platform.isIOS) {
      return 'libtorca_bridge.dylib';
    }
    throw UnsupportedError('Torca native runtime is unsupported on this platform');
  }

  final _NativeBindings _bindings;
  final _EngineHandle _handle;
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto());
  bool _disposed = false;

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  Future<BridgeResultDto> initialize() async {
    return _refreshSnapshot();
  }

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    if (_disposed) {
      return const BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'native engine gateway is disposed',
      );
    }

    if (command is RefreshSnapshotCommandDto) {
      return _refreshSnapshot();
    }

    if (command is CreateIdentityCommandDto) {
      final _NativeUtf8 identityId = _NativeUtf8(_bindings, command.identityIdHex);
      final _NativeUtf8 displayName = _NativeUtf8(_bindings, command.displayName);
      try {
        _bindings.createIdentity(
          _handle,
          identityId.pointer,
          identityId.length,
          displayName.pointer,
          displayName.length,
          command.atMs,
        );
      } finally {
        identityId.dispose();
        displayName.dispose();
      }
    } else if (command is StartPairingCommandDto) {
      final _NativeUtf8 sessionId = _NativeUtf8(_bindings, command.sessionIdHex);
      final _NativeUtf8 code = _NativeUtf8(_bindings, command.code);
      try {
        _bindings.startPairing(
          _handle,
          sessionId.pointer,
          sessionId.length,
          code.pointer,
          code.length,
          command.expiresAtMs,
        );
      } finally {
        sessionId.dispose();
        code.dispose();
      }
    } else if (command is QueueMessageCommandDto) {
      final _NativeUtf8 messageId = _NativeUtf8(_bindings, command.messageIdHex);
      final _NativeUtf8 conversationId = _NativeUtf8(
        _bindings,
        command.conversationIdHex,
      );
      final _NativeUtf8 body = _NativeUtf8(_bindings, command.body);
      try {
        _bindings.queueMessage(
          _handle,
          messageId.pointer,
          messageId.length,
          conversationId.pointer,
          conversationId.length,
          body.pointer,
          body.length,
          command.atMs,
        );
      } finally {
        messageId.dispose();
        conversationId.dispose();
        body.dispose();
      }
    } else {
      return const BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'unsupported bridge command',
      );
    }

    final BridgeResultDto result = _decodeResult(_readResultJson());
    if (result.ok) {
      await _refreshSnapshot();
    }
    return result;
  }

  Future<BridgeResultDto> _refreshSnapshot() async {
    final int status = _bindings.refreshSnapshot(_handle);
    if (status != 0) {
      return _decodeResult(_readResultJson());
    }
    _snapshots.value = _decodeSnapshot(_readSnapshotJson());
    return const BridgeResultDto(ok: true, kind: 'snapshot');
  }

  String _readResultJson() => _readNativeString(
        _bindings.resultPointer(_handle),
        _bindings.resultLength(_handle),
      );

  String _readSnapshotJson() => _readNativeString(
        _bindings.snapshotPointer(_handle),
        _bindings.snapshotLength(_handle),
      );

  String _readNativeString(ffi.Pointer<ffi.Uint8> pointer, int length) {
    if (pointer == ffi.nullptr || length == 0) {
      return '';
    }
    return utf8.decode(pointer.asTypedList(length), allowMalformed: false);
  }

  BridgeResultDto _decodeResult(String json) {
    final Object? decoded = jsonDecode(json);
    final Map<String, Object?> map = _stringMap(decoded, 'bridge result');
    return BridgeResultDto(
      ok: _bool(map, 'ok'),
      kind: _string(map, 'kind'),
      error: _optionalString(map, 'error'),
    );
  }

  AppSnapshotDto _decodeSnapshot(String json) {
    final Object? decoded = jsonDecode(json);
    final Map<String, Object?> map = _stringMap(decoded, 'app snapshot');
    final int version = _int(map, 'contractVersion');
    if (version != torcaContractVersion) {
      throw FormatException('unsupported native contract version $version');
    }

    final Object? identityValue = map['identity'];
    final IdentityDto? identity = identityValue == null
        ? null
        : IdentityDto(
            displayName: _string(
              _stringMap(identityValue, 'identity'),
              'displayName',
            ),
          );

    return AppSnapshotDto(
      identity: identity,
      contacts: _list(map, 'contacts')
          .map<ContactDto>((Object? value) {
            final Map<String, Object?> item = _stringMap(value, 'contact');
            return ContactDto(
              id: _string(item, 'id'),
              onionAddress: _string(item, 'onionAddress'),
              status: _string(item, 'status'),
            );
          })
          .toList(growable: false),
      conversations: _list(map, 'conversations')
          .map<ConversationDto>((Object? value) {
            final Map<String, Object?> item = _stringMap(value, 'conversation');
            return ConversationDto(
              id: _string(item, 'id'),
              contactId: _string(item, 'contactId'),
              status: _string(item, 'status'),
            );
          })
          .toList(growable: false),
      messages: _list(map, 'messages')
          .map<MessageDto>((Object? value) {
            final Map<String, Object?> item = _stringMap(value, 'message');
            return MessageDto(
              id: _string(item, 'id'),
              conversationId: _string(item, 'conversationId'),
              body: _string(item, 'body'),
              direction: _string(item, 'direction'),
              status: _string(item, 'status'),
            );
          })
          .toList(growable: false),
    );
  }

  Map<String, Object?> _stringMap(Object? value, String field) {
    if (value is! Map<Object?, Object?>) {
      throw FormatException('$field must be a map');
    }
    return value.map<String, Object?>((Object? key, Object? item) {
      if (key is! String) {
        throw FormatException('$field contains a non-string key');
      }
      return MapEntry<String, Object?>(key, item);
    });
  }

  List<Object?> _list(Map<String, Object?> map, String field) {
    final Object? value = map[field];
    if (value is List<Object?>) {
      return value;
    }
    throw FormatException('$field must be a list');
  }

  String _string(Map<String, Object?> map, String field) {
    final Object? value = map[field];
    if (value is String) {
      return value;
    }
    throw FormatException('$field must be a string');
  }

  String? _optionalString(Map<String, Object?> map, String field) {
    final Object? value = map[field];
    if (value == null || value is String) {
      return value as String?;
    }
    throw FormatException('$field must be a string or null');
  }

  bool _bool(Map<String, Object?> map, String field) {
    final Object? value = map[field];
    if (value is bool) {
      return value;
    }
    throw FormatException('$field must be a bool');
  }

  int _int(Map<String, Object?> map, String field) {
    final Object? value = map[field];
    if (value is int) {
      return value;
    }
    throw FormatException('$field must be an int');
  }

  @override
  Future<void> dispose() async {
    if (_disposed) {
      return;
    }
    _disposed = true;
    _bindings.close(_handle);
    _bindings.engineDestroy(_handle);
    _snapshots.dispose();
  }
}

class _NativeUtf8 {
  _NativeUtf8(this._bindings, String value) : _bytes = utf8.encode(value) {
    pointer = _bindings.alloc(_bytes.length);
    if (_bytes.isNotEmpty) {
      if (pointer == ffi.nullptr) {
        throw StateError('native UTF-8 allocation failed');
      }
      pointer.asTypedList(_bytes.length).setAll(0, _bytes);
    }
  }

  final _NativeBindings _bindings;
  final List<int> _bytes;
  late final ffi.Pointer<ffi.Uint8> pointer;

  int get length => _bytes.length;

  void dispose() {
    _bindings.free(pointer, _bytes.length);
  }
}

class _NativeBindings {
  _NativeBindings(ffi.DynamicLibrary library)
      : contractVersion = library.lookupFunction<
            _ContractVersionNative,
            _ContractVersionDart
          >('torca_contract_version'),
        alloc = library.lookupFunction<_AllocNative, _AllocDart>('torca_alloc'),
        free = library.lookupFunction<_FreeNative, _FreeDart>('torca_free'),
        engineNew = library.lookupFunction<_EngineNewNative, _EngineNewDart>(
          'torca_engine_new',
        ),
        engineDestroy = library.lookupFunction<
            _EngineDestroyNative,
            _EngineDestroyDart
          >('torca_engine_destroy'),
        createIdentity = library.lookupFunction<
            _CreateIdentityNative,
            _CreateIdentityDart
          >('torca_engine_create_identity'),
        startPairing = library.lookupFunction<
            _StartPairingNative,
            _StartPairingDart
          >('torca_engine_start_pairing'),
        queueMessage = library.lookupFunction<
            _QueueMessageNative,
            _QueueMessageDart
          >('torca_engine_queue_message'),
        refreshSnapshot = library.lookupFunction<
            _RefreshSnapshotNative,
            _RefreshSnapshotDart
          >('torca_engine_refresh_snapshot'),
        resultPointer = library.lookupFunction<
            _BufferPointerNative,
            _BufferPointerDart
          >('torca_engine_result_ptr'),
        resultLength = library.lookupFunction<
            _BufferLengthNative,
            _BufferLengthDart
          >('torca_engine_result_len'),
        snapshotPointer = library.lookupFunction<
            _BufferPointerNative,
            _BufferPointerDart
          >('torca_engine_snapshot_ptr'),
        snapshotLength = library.lookupFunction<
            _BufferLengthNative,
            _BufferLengthDart
          >('torca_engine_snapshot_len'),
        close = library.lookupFunction<_CloseNative, _CloseDart>(
          'torca_engine_close',
        );

  final _ContractVersionDart contractVersion;
  final _AllocDart alloc;
  final _FreeDart free;
  final _EngineNewDart engineNew;
  final _EngineDestroyDart engineDestroy;
  final _CreateIdentityDart createIdentity;
  final _StartPairingDart startPairing;
  final _QueueMessageDart queueMessage;
  final _RefreshSnapshotDart refreshSnapshot;
  final _BufferPointerDart resultPointer;
  final _BufferLengthDart resultLength;
  final _BufferPointerDart snapshotPointer;
  final _BufferLengthDart snapshotLength;
  final _CloseDart close;
}
