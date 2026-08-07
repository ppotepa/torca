import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';
import 'engine_gateway.dart';

typedef _Handle = ffi.Pointer<ffi.Void>;
typedef _EngineNewNative = _Handle Function();
typedef _EngineNewDart = _Handle Function();
typedef _EngineDestroyNative = ffi.Void Function(_Handle);
typedef _EngineDestroyDart = void Function(_Handle);
typedef _ContractVersionNative = ffi.Uint16 Function();
typedef _ContractVersionDart = int Function();
typedef _AllocNative = ffi.Pointer<ffi.Uint8> Function(ffi.UintPtr);
typedef _AllocDart = ffi.Pointer<ffi.Uint8> Function(int);
typedef _FreeNative = ffi.Void Function(ffi.Pointer<ffi.Uint8>, ffi.UintPtr);
typedef _FreeDart = void Function(ffi.Pointer<ffi.Uint8>, int);
typedef _NoArgCommandNative = ffi.Int32 Function(_Handle);
typedef _NoArgCommandDart = int Function(_Handle);
typedef _IdNative = ffi.Int32 Function(_Handle, ffi.Pointer<ffi.Uint8>, ffi.UintPtr);
typedef _IdDart = int Function(_Handle, ffi.Pointer<ffi.Uint8>, int);
typedef _TwoStringsNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
);
typedef _TwoStringsDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
);
typedef _QueueMessageIntentNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
);
typedef _QueueMessageIntentDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
);
typedef _QueueAttachmentIntentNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>,
  ffi.UintPtr,
  ffi.Uint64,
);
typedef _QueueAttachmentIntentDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
  ffi.Pointer<ffi.Uint8>,
  int,
  int,
);
typedef _RefreshNative = ffi.Int32 Function(_Handle);
typedef _RefreshDart = int Function(_Handle);
typedef _PointerNative = ffi.Pointer<ffi.Uint8> Function(_Handle);
typedef _PointerDart = ffi.Pointer<ffi.Uint8> Function(_Handle);
typedef _LengthNative = ffi.UintPtr Function(_Handle);
typedef _LengthDart = int Function(_Handle);

class FfiEngineGateway implements EngineGateway {
  FfiEngineGateway._(this._bindings, this._handle) {
    _poller = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!_disposed) unawaited(_refreshSnapshot(silent: true));
    });
  }

  static FfiEngineGateway open({ffi.DynamicLibrary? library}) {
    final bindings = _NativeBindings(library ?? ffi.DynamicLibrary.open(_libraryName()));
    final version = bindings.contractVersion();
    if (version != torcaContractVersion) {
      throw StateError(
        'native Torca contract $version does not match Flutter contract $torcaContractVersion',
      );
    }
    final handle = bindings.engineNew();
    if (handle == ffi.nullptr) {
      throw StateError('native Torca process handle could not be acquired');
    }
    return FfiEngineGateway._(bindings, handle);
  }

  static String _libraryName() {
    if (Platform.isWindows) return 'torca_bridge.dll';
    if (Platform.isAndroid || Platform.isLinux) return 'libtorca_bridge.so';
    if (Platform.isMacOS || Platform.isIOS) return 'libtorca_bridge.dylib';
    throw UnsupportedError('Torca native runtime is unsupported on this platform');
  }

  final _NativeBindings _bindings;
  final _Handle _handle;
  final ValueNotifier<AppSnapshotDto> _snapshots = ValueNotifier(const AppSnapshotDto());
  late final Timer _poller;
  bool _disposed = false;

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  Future<BridgeResultDto> initialize() => _refreshSnapshot();

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    if (_disposed) {
      return const BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'native engine gateway is disposed',
      );
    }
    if (command is RefreshSnapshotCommandDto) return _refreshSnapshot();

    if (command is CreateIdentityCommandDto) {
      _withString(command.displayName, _bindings.createIdentityIntent);
    } else if (command is CreatePairingCommandDto) {
      _bindings.createPairingIntent(_handle);
    } else if (command is JoinPairingCommandDto) {
      _withString(command.code, _bindings.joinPairingIntent);
    } else if (command is ApprovePairingCommandDto) {
      _withString(command.sessionIdHex, _bindings.approvePairing);
    } else if (command is RejectPairingCommandDto) {
      _withString(command.sessionIdHex, _bindings.rejectPairing);
    } else if (command is CancelPairingCommandDto) {
      _withString(command.sessionIdHex, _bindings.cancelPairing);
    } else if (command is RenameContactCommandDto) {
      _withTwoStrings(command.contactIdHex, command.displayName, _bindings.renameContact);
    } else if (command is BlockContactCommandDto) {
      _withString(command.contactIdHex, _bindings.blockContact);
    } else if (command is UnblockContactCommandDto) {
      _withString(command.contactIdHex, _bindings.unblockContact);
    } else if (command is RemoveContactCommandDto) {
      _withString(command.contactIdHex, _bindings.removeContact);
    } else if (command is ClearConversationHistoryCommandDto) {
      _withString(command.conversationIdHex, _bindings.clearConversationHistory);
    } else if (command is QueueMessageCommandDto) {
      _queueMessageIntent(command);
    } else if (command is RetryMessageCommandDto) {
      _withString(command.messageIdHex, _bindings.retryMessageIntent);
    } else if (command is MarkConversationReadCommandDto) {
      _withString(command.conversationIdHex, _bindings.markConversationRead);
    } else if (command is QueueAttachmentCommandDto) {
      _queueAttachmentIntent(command);
    } else if (command is RetryAttachmentCommandDto) {
      _withString(command.attachmentIdHex, _bindings.retryAttachment);
    } else if (command is CancelAttachmentCommandDto) {
      _withString(command.attachmentIdHex, _bindings.cancelAttachment);
    } else if (command is ExportAttachmentCommandDto) {
      _withTwoStrings(
        command.attachmentIdHex,
        command.destinationPath,
        _bindings.exportAttachment,
      );
    } else {
      return const BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'unsupported bridge command',
      );
    }

    final result = _decodeResult(_readResultJson());
    if (result.ok) await _refreshSnapshot(silent: true);
    return result;
  }

  void _queueMessageIntent(QueueMessageCommandDto command) {
    final conversation = _NativeUtf8(_bindings, command.conversationIdHex);
    final body = _NativeUtf8(_bindings, command.body);
    final reply = _NativeUtf8(_bindings, command.replyToMessageId ?? '');
    try {
      _bindings.queueMessageIntent(
        _handle,
        conversation.pointer,
        conversation.length,
        body.pointer,
        body.length,
        reply.pointer,
        reply.length,
      );
    } finally {
      conversation.dispose();
      body.dispose();
      reply.dispose();
    }
  }

  void _queueAttachmentIntent(QueueAttachmentCommandDto command) {
    final conversation = _NativeUtf8(_bindings, command.conversationIdHex);
    final path = _NativeUtf8(_bindings, command.sourcePath);
    final name = _NativeUtf8(_bindings, command.name);
    final mediaType = _NativeUtf8(_bindings, command.mediaType);
    try {
      _bindings.queueAttachmentIntent(
        _handle,
        conversation.pointer,
        conversation.length,
        path.pointer,
        path.length,
        name.pointer,
        name.length,
        mediaType.pointer,
        mediaType.length,
        command.size,
      );
    } finally {
      conversation.dispose();
      path.dispose();
      name.dispose();
      mediaType.dispose();
    }
  }

  void _withString(String value, _IdDart operation) {
    final native = _NativeUtf8(_bindings, value);
    try {
      operation(_handle, native.pointer, native.length);
    } finally {
      native.dispose();
    }
  }

  void _withTwoStrings(String first, String second, _TwoStringsDart operation) {
    final firstValue = _NativeUtf8(_bindings, first);
    final secondValue = _NativeUtf8(_bindings, second);
    try {
      operation(
        _handle,
        firstValue.pointer,
        firstValue.length,
        secondValue.pointer,
        secondValue.length,
      );
    } finally {
      firstValue.dispose();
      secondValue.dispose();
    }
  }

  @override
  Future<String> diagnosticsJson() async {
    if (_disposed) return '{"events":[]}';
    if (_bindings.refreshDiagnostics(_handle) != 0) return '{"events":[]}';
    return _readNativeString(
      _bindings.diagnosticsPointer(_handle),
      _bindings.diagnosticsLength(_handle),
    );
  }

  Future<BridgeResultDto> _refreshSnapshot({bool silent = false}) async {
    final status = _bindings.refreshSnapshot(_handle);
    if (status != 0) {
      final error = _decodeResult(_readResultJson());
      return silent ? const BridgeResultDto(ok: false, kind: 'error') : error;
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
    if (pointer == ffi.nullptr || length == 0) return '';
    return utf8.decode(pointer.asTypedList(length), allowMalformed: false);
  }

  BridgeResultDto _decodeResult(String json) {
    if (json.isEmpty) {
      return const BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'native runtime returned an empty result',
      );
    }
    final map = _map(jsonDecode(json), 'bridge result');
    return BridgeResultDto(
      ok: _bool(map, 'ok'),
      kind: _string(map, 'kind'),
      error: _optionalString(map, 'error'),
    );
  }

  AppSnapshotDto _decodeSnapshot(String json) {
    final map = _map(jsonDecode(json), 'app snapshot');
    final version = _int(map, 'contractVersion');
    if (version != torcaContractVersion) {
      throw FormatException('unsupported native contract version $version');
    }
    final identityValue = map['identity'];
    final identity = identityValue == null
        ? null
        : IdentityDto(
            displayName: _string(_map(identityValue, 'identity'), 'displayName'),
          );
    return AppSnapshotDto(
      identity: identity,
      torState: _string(map, 'torState'),
      onionAddress: _optionalString(map, 'onionAddress'),
      pairings: _list(map, 'pairings').map((value) {
        final item = _map(value, 'pairing');
        return PairingDto(
          id: _string(item, 'id'),
          code: _string(item, 'code'),
          role: _string(item, 'role'),
          state: _string(item, 'state'),
          expiresAtMs: _int(item, 'expiresAtMs'),
          localApproved: _bool(item, 'localApproved'),
          remoteApproved: _bool(item, 'remoteApproved'),
        );
      }).toList(growable: false),
      contacts: _list(map, 'contacts').map((value) {
        final item = _map(value, 'contact');
        final peerHealth = _map(item['peerHealth'], 'contact.peerHealth');
        return ContactDto(
          id: _string(item, 'id'),
          displayName: _string(item, 'displayName'),
          onionAddress: _string(item, 'onionAddress'),
          status: _string(item, 'status'),
          connectionState: _string(item, 'connectionState'),
          safetyNumber: _optionalString(item, 'safetyNumber'),
          peerHealth: PeerHealthDto(
            state: _string(peerHealth, 'state'),
            quality: _string(peerHealth, 'quality'),
            rttMs: _optionalInt(peerHealth, 'rttMs'),
            lastSuccessAtMs: _optionalInt(peerHealth, 'lastSuccessAtMs'),
            consecutiveFailures: _int(peerHealth, 'consecutiveFailures'),
            reconnectAttempt: _int(peerHealth, 'reconnectAttempt'),
          ),
        );
      }).toList(growable: false),
      conversations: _list(map, 'conversations').map((value) {
        final item = _map(value, 'conversation');
        return ConversationDto(
          id: _string(item, 'id'),
          contactId: _string(item, 'contactId'),
          status: _string(item, 'status'),
        );
      }).toList(growable: false),
      messages: _list(map, 'messages').map((value) {
        final item = _map(value, 'message');
        return MessageDto(
          id: _string(item, 'id'),
          conversationId: _string(item, 'conversationId'),
          body: _string(item, 'body'),
          direction: _string(item, 'direction'),
          status: _string(item, 'status'),
          replyToMessageId: _optionalString(item, 'replyToMessageId'),
          createdAtMs: _int(item, 'createdAtMs'),
          updatedAtMs: _int(item, 'updatedAtMs'),
          attemptCount: _int(item, 'attemptCount'),
        );
      }).toList(growable: false),
      attachments: _list(map, 'attachments').map((value) {
        final item = _map(value, 'attachment');
        return AttachmentDto(
          id: _string(item, 'id'),
          messageId: _string(item, 'messageId'),
          name: _string(item, 'name'),
          mediaType: _string(item, 'mediaType'),
          size: _int(item, 'size'),
          status: _string(item, 'status'),
          offset: _int(item, 'offset'),
        );
      }).toList(growable: false),
    );
  }

  Map<String, Object?> _map(Object? value, String field) {
    if (value is! Map<Object?, Object?>) {
      throw FormatException('$field must be a map');
    }
    return value.map((key, item) {
      if (key is! String) throw FormatException('$field contains a non-string key');
      return MapEntry<String, Object?>(key, item);
    });
  }

  List<Object?> _list(Map<String, Object?> map, String field) {
    final value = map[field];
    if (value is List<Object?>) return value;
    throw FormatException('$field must be a list');
  }

  String _string(Map<String, Object?> map, String field) {
    final value = map[field];
    if (value is String) return value;
    throw FormatException('$field must be a string');
  }

  String? _optionalString(Map<String, Object?> map, String field) {
    final value = map[field];
    if (value == null || value is String) return value as String?;
    throw FormatException('$field must be a string or null');
  }

  bool _bool(Map<String, Object?> map, String field) {
    final value = map[field];
    if (value is bool) return value;
    throw FormatException('$field must be a bool');
  }

  int _int(Map<String, Object?> map, String field) {
    final value = map[field];
    if (value is int) return value;
    throw FormatException('$field must be an int');
  }

  int? _optionalInt(Map<String, Object?> map, String field) {
    final value = map[field];
    if (value == null || value is int) return value as int?;
    throw FormatException('$field must be an int or null');
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _poller.cancel();
    _bindings.engineDestroy(_handle);
    _snapshots.dispose();
  }
}

class _NativeUtf8 {
  _NativeUtf8(this._bindings, String value) : _bytes = utf8.encode(value) {
    pointer = _bindings.alloc(_bytes.length);
    if (_bytes.isNotEmpty) {
      if (pointer == ffi.nullptr) throw StateError('native UTF-8 allocation failed');
      pointer.asTypedList(_bytes.length).setAll(0, _bytes);
    }
  }

  final _NativeBindings _bindings;
  final List<int> _bytes;
  late final ffi.Pointer<ffi.Uint8> pointer;

  int get length => _bytes.length;
  void dispose() => _bindings.free(pointer, _bytes.length);
}

class _NativeBindings {
  _NativeBindings(ffi.DynamicLibrary library)
      : contractVersion = library.lookupFunction<_ContractVersionNative, _ContractVersionDart>('torca_contract_version'),
        alloc = library.lookupFunction<_AllocNative, _AllocDart>('torca_alloc'),
        free = library.lookupFunction<_FreeNative, _FreeDart>('torca_free'),
        engineNew = library.lookupFunction<_EngineNewNative, _EngineNewDart>('torca_engine_new'),
        engineDestroy = library.lookupFunction<_EngineDestroyNative, _EngineDestroyDart>('torca_engine_destroy'),
        createIdentityIntent = library.lookupFunction<_IdNative, _IdDart>('torca_engine_create_identity_intent'),
        createPairingIntent = library.lookupFunction<_NoArgCommandNative, _NoArgCommandDart>('torca_engine_create_pairing_intent'),
        joinPairingIntent = library.lookupFunction<_IdNative, _IdDart>('torca_engine_join_pairing_intent'),
        approvePairing = library.lookupFunction<_IdNative, _IdDart>('torca_engine_approve_pairing'),
        rejectPairing = library.lookupFunction<_IdNative, _IdDart>('torca_engine_reject_pairing'),
        cancelPairing = library.lookupFunction<_IdNative, _IdDart>('torca_engine_cancel_pairing'),
        renameContact = library.lookupFunction<_TwoStringsNative, _TwoStringsDart>('torca_engine_rename_contact'),
        blockContact = library.lookupFunction<_IdNative, _IdDart>('torca_engine_block_contact'),
        unblockContact = library.lookupFunction<_IdNative, _IdDart>('torca_engine_unblock_contact'),
        removeContact = library.lookupFunction<_IdNative, _IdDart>('torca_engine_remove_contact'),
        clearConversationHistory = library.lookupFunction<_IdNative, _IdDart>('torca_engine_clear_conversation_history'),
        queueMessageIntent = library.lookupFunction<_QueueMessageIntentNative, _QueueMessageIntentDart>('torca_engine_queue_message_intent'),
        retryMessageIntent = library.lookupFunction<_IdNative, _IdDart>('torca_engine_retry_message_intent'),
        markConversationRead = library.lookupFunction<_IdNative, _IdDart>('torca_engine_mark_conversation_read'),
        queueAttachmentIntent = library.lookupFunction<_QueueAttachmentIntentNative, _QueueAttachmentIntentDart>('torca_engine_queue_attachment_intent'),
        retryAttachment = library.lookupFunction<_IdNative, _IdDart>('torca_engine_retry_attachment'),
        cancelAttachment = library.lookupFunction<_IdNative, _IdDart>('torca_engine_cancel_attachment'),
        exportAttachment = library.lookupFunction<_TwoStringsNative, _TwoStringsDart>('torca_engine_export_attachment'),
        refreshSnapshot = library.lookupFunction<_RefreshNative, _RefreshDart>('torca_engine_refresh_snapshot'),
        refreshDiagnostics = library.lookupFunction<_RefreshNative, _RefreshDart>('torca_engine_refresh_diagnostics'),
        resultPointer = library.lookupFunction<_PointerNative, _PointerDart>('torca_engine_result_ptr'),
        resultLength = library.lookupFunction<_LengthNative, _LengthDart>('torca_engine_result_len'),
        snapshotPointer = library.lookupFunction<_PointerNative, _PointerDart>('torca_engine_snapshot_ptr'),
        snapshotLength = library.lookupFunction<_LengthNative, _LengthDart>('torca_engine_snapshot_len'),
        diagnosticsPointer = library.lookupFunction<_PointerNative, _PointerDart>('torca_engine_diagnostics_ptr'),
        diagnosticsLength = library.lookupFunction<_LengthNative, _LengthDart>('torca_engine_diagnostics_len');

  final _ContractVersionDart contractVersion;
  final _AllocDart alloc;
  final _FreeDart free;
  final _EngineNewDart engineNew;
  final _EngineDestroyDart engineDestroy;
  final _IdDart createIdentityIntent;
  final _NoArgCommandDart createPairingIntent;
  final _IdDart joinPairingIntent;
  final _IdDart approvePairing;
  final _IdDart rejectPairing;
  final _IdDart cancelPairing;
  final _TwoStringsDart renameContact;
  final _IdDart blockContact;
  final _IdDart unblockContact;
  final _IdDart removeContact;
  final _IdDart clearConversationHistory;
  final _QueueMessageIntentDart queueMessageIntent;
  final _IdDart retryMessageIntent;
  final _IdDart markConversationRead;
  final _QueueAttachmentIntentDart queueAttachmentIntent;
  final _IdDart retryAttachment;
  final _IdDart cancelAttachment;
  final _TwoStringsDart exportAttachment;
  final _RefreshDart refreshSnapshot;
  final _RefreshDart refreshDiagnostics;
  final _PointerDart resultPointer;
  final _LengthDart resultLength;
  final _PointerDart snapshotPointer;
  final _LengthDart snapshotLength;
  final _PointerDart diagnosticsPointer;
  final _LengthDart diagnosticsLength;
}
