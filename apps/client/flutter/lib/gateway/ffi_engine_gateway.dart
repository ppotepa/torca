import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'dart:isolate';

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
typedef _MaxAttachmentNative = ffi.Uint64 Function();
typedef _MaxAttachmentDart = int Function();
typedef _AllocNative = ffi.Pointer<ffi.Uint8> Function(ffi.UintPtr);
typedef _AllocDart = ffi.Pointer<ffi.Uint8> Function(int);
typedef _FreeNative = ffi.Void Function(ffi.Pointer<ffi.Uint8>, ffi.UintPtr);
typedef _FreeDart = void Function(ffi.Pointer<ffi.Uint8>, int);
typedef _NoArgNative = ffi.Int32 Function(_Handle);
typedef _NoArgDart = int Function(_Handle);
typedef _OneStringNative = ffi.Int32 Function(_Handle, ffi.Pointer<ffi.Uint8>, ffi.UintPtr);
typedef _OneStringDart = int Function(_Handle, ffi.Pointer<ffi.Uint8>, int);
typedef _ReadIntentNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Uint8,
);
typedef _ReadIntentDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, int,
  int,
);
typedef _TwoStringsNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
);
typedef _TwoStringsDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
);
typedef _MessageIntentNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
);
typedef _MessageIntentDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
);
typedef _AttachmentIntentNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Uint64,
);
typedef _AttachmentIntentDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  int,
);
typedef _HistoryPageNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Int64,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Uint32,
);
typedef _HistoryPageDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, int,
  int,
  ffi.Pointer<ffi.Uint8>, int,
  int,
);
typedef _HistorySearchNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Uint32,
);
typedef _HistorySearchDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  int,
);
typedef _RefreshNative = ffi.Int32 Function(_Handle);
typedef _RefreshDart = int Function(_Handle);
typedef _PointerNative = ffi.Pointer<ffi.Uint8> Function(_Handle);
typedef _PointerDart = ffi.Pointer<ffi.Uint8> Function(_Handle);
typedef _LengthNative = ffi.UintPtr Function(_Handle);
typedef _LengthDart = int Function(_Handle);

class FfiEngineGateway
    implements EngineGateway, AttachmentCapabilitiesProvider, ConversationHistoryProvider {
  FfiEngineGateway._(this._bindings, this._handle, this.capabilities) {
    _poller = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!_disposed && !_nativeOperationInFlight) {
        unawaited(_refreshSnapshot(silent: true));
      }
    });
  }

  static FfiEngineGateway open({ffi.DynamicLibrary? library}) {
    final bindings = _NativeBindings(library ?? ffi.DynamicLibrary.open(_nativeLibraryName()));
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
    return FfiEngineGateway._(
      bindings,
      handle,
      AppCapabilities(maxAttachmentBytes: bindings.maxAttachmentBytes()),
    );
  }

  final _NativeBindings _bindings;
  final _Handle _handle;
  @override
  final AppCapabilities capabilities;
  final ValueNotifier<AppSnapshotDto> _snapshots = ValueNotifier(const AppSnapshotDto());
  late final Timer _poller;
  Future<void> _nativeTail = Future<void>.value();
  String _lastDiagnostics = '{"events":[]}';
  bool _nativeOperationInFlight = false;
  bool _disposed = false;

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  Future<BridgeResultDto> initialize() => _refreshSnapshot();

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    if (_disposed) return _runtimeUnavailable();
    if (command is RefreshSnapshotCommandDto) return _refreshSnapshot();
    final call = _encodeNativeCall(command);
    if (call == null) {
      return const BridgeResultDto(
        ok: false,
        kind: 'error:invalid_input',
        error: 'The supplied value is not valid.',
      );
    }
    final result = await _serializedNative(() async {
      final resultJson = await Isolate.run(() => _executeNativeCall(call));
      return _decodeResult(resultJson);
    });
    if (result.ok && !_disposed) await _refreshSnapshot(silent: true);
    return result;
  }

  @override
  Future<ConversationPageDto> loadConversationPage(
    String conversationId, {
    MessageDto? before,
    int limit = 100,
  }) =>
      _serializedNative(() async {
        final json = await Isolate.run(
          () => _executeNativeCall(<Object?>[
            'history_page',
            conversationId,
            before?.createdAtMs ?? -1,
            before?.id ?? '',
            limit.clamp(1, 200),
          ]),
        );
        return _decodeConversationPage(json);
      });

  @override
  Future<ConversationPageDto> searchConversation(
    String conversationId,
    String query, {
    int limit = 100,
  }) async {
    if (query.trim().isEmpty) {
      return const ConversationPageDto(messages: <MessageDto>[], hasMore: false);
    }
    return _serializedNative(() async {
      final json = await Isolate.run(
        () => _executeNativeCall(<Object?>[
          'history_search',
          conversationId,
          query,
          limit.clamp(1, 200),
        ]),
      );
      return _decodeConversationPage(json);
    });
  }

  Future<T> _serializedNative<T>(Future<T> Function() action) async {
    final previous = _nativeTail;
    final release = Completer<void>();
    _nativeTail = release.future;
    await previous;
    if (_disposed) {
      release.complete();
      throw StateError('native engine gateway is disposed');
    }
    _nativeOperationInFlight = true;
    try {
      return await action();
    } finally {
      _nativeOperationInFlight = false;
      release.complete();
    }
  }

  @override
  Future<String> diagnosticsJson() async {
    if (_disposed || _nativeOperationInFlight) return _lastDiagnostics;
    if (_bindings.refreshDiagnostics(_handle) != 0) return _lastDiagnostics;
    _lastDiagnostics = _readNativeString(
      _bindings.diagnosticsPointer(_handle),
      _bindings.diagnosticsLength(_handle),
    );
    return _lastDiagnostics;
  }

  Future<BridgeResultDto> _refreshSnapshot({bool silent = false}) async {
    if (_disposed) return _runtimeUnavailable();
    if (_nativeOperationInFlight) {
      return silent
          ? const BridgeResultDto(ok: false, kind: 'busy')
          : const BridgeResultDto(ok: true, kind: 'snapshot_cached');
    }
    final status = _bindings.refreshSnapshot(_handle);
    if (status != 0) {
      final error = _decodeResult(_readResultJson());
      return silent
          ? const BridgeResultDto(ok: false, kind: 'error:runtime_unavailable')
          : error;
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

  BridgeResultDto _decodeResult(String json) {
    if (json.isEmpty) return _runtimeUnavailable();
    final map = _map(jsonDecode(json), 'bridge result');
    return BridgeResultDto(
      ok: _bool(map, 'ok'),
      kind: _string(map, 'kind'),
      error: _optionalString(map, 'error'),
    );
  }

  ConversationPageDto _decodeConversationPage(String json) {
    final map = _map(jsonDecode(json), 'conversation page');
    final messages = _list(map, 'messages')
        .map((value) => _decodeMessage(_map(value, 'message')))
        .toList(growable: false);
    return ConversationPageDto(
      messages: messages,
      hasMore: _bool(map, 'hasMore'),
    );
  }

  AppSnapshotDto _decodeSnapshot(String json) {
    final map = _map(jsonDecode(json), 'app snapshot');
    final version = _int(map, 'contractVersion');
    if (version != torcaContractVersion) {
      throw FormatException('unsupported native contract version $version');
    }
    final identityValue = map['identity'];
    return AppSnapshotDto(
      identity: identityValue == null
          ? null
          : IdentityDto(
              displayName: _string(_map(identityValue, 'identity'), 'displayName'),
            ),
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
        final health = _map(item['peerHealth'], 'contact.peerHealth');
        return ContactDto(
          id: _string(item, 'id'),
          displayName: _string(item, 'displayName'),
          onionAddress: _string(item, 'onionAddress'),
          status: _string(item, 'status'),
          connectionState: _string(item, 'connectionState'),
          safetyNumber: _optionalString(item, 'safetyNumber'),
          peerHealth: PeerHealthDto(
            state: _string(health, 'state'),
            quality: _string(health, 'quality'),
            rttMs: _optionalInt(health, 'rttMs'),
            lastSuccessAtMs: _optionalInt(health, 'lastSuccessAtMs'),
            consecutiveFailures: _int(health, 'consecutiveFailures'),
            reconnectAttempt: _int(health, 'reconnectAttempt'),
          ),
          verificationStatus: _stringOr(item, 'verificationStatus', 'unverified'),
          verifiedAtMs: _optionalInt(item, 'verifiedAtMs'),
        );
      }).toList(growable: false),
      conversations: _list(map, 'conversations').map((value) {
        final item = _map(value, 'conversation');
        return ConversationDto(
          id: _string(item, 'id'),
          contactId: _string(item, 'contactId'),
          status: _string(item, 'status'),
          unreadCount: _intOr(item, 'unreadCount', 0),
          lastActivityAtMs: _intOr(item, 'lastActivityAtMs', 0),
          lastMessageBody: _optionalString(item, 'lastMessageBody'),
          lastMessageDirection: _optionalString(item, 'lastMessageDirection'),
          lastMessageStatus: _optionalString(item, 'lastMessageStatus'),
        );
      }).toList(growable: false),
      messages: _list(map, 'messages')
          .map((value) => _decodeMessage(_map(value, 'message')))
          .toList(growable: false),
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

  MessageDto _decodeMessage(Map<String, Object?> item) => MessageDto(
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

  Map<String, Object?> _map(Object? value, String field) {
    if (value is! Map<Object?, Object?>) throw FormatException('$field must be a map');
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
  String _stringOr(Map<String, Object?> map, String field, String fallback) {
    final value = map[field];
    return value is String ? value : fallback;
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
  int _intOr(Map<String, Object?> map, String field, int fallback) {
    final value = map[field];
    return value is int ? value : fallback;
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
    await _nativeTail;
    _bindings.engineDestroy(_handle);
    _snapshots.dispose();
  }

  static const BridgeResultDto _runtimeUnavailableResult = BridgeResultDto(
    ok: false,
    kind: 'error:runtime_unavailable',
    error: 'The secure Torca runtime is currently unavailable.',
  );
  BridgeResultDto _runtimeUnavailable() => _runtimeUnavailableResult;
}

List<Object?>? _encodeNativeCall(BridgeCommandDto command) {
  if (command is CreateIdentityCommandDto) return <Object?>['create_identity', command.displayName];
  if (command is CreatePairingCommandDto) return const <Object?>['create_pairing'];
  if (command is JoinPairingCommandDto) return <Object?>['join_pairing', command.code];
  if (command is ApprovePairingCommandDto) return <Object?>['approve_pairing', command.sessionIdHex];
  if (command is RejectPairingCommandDto) return <Object?>['reject_pairing', command.sessionIdHex];
  if (command is CancelPairingCommandDto) return <Object?>['cancel_pairing', command.sessionIdHex];
  if (command is RenameContactCommandDto) return <Object?>['rename_contact', command.contactIdHex, command.displayName];
  if (command is VerifyContactCommandDto) return <Object?>['verify_contact', command.contactIdHex];
  if (command is ResetContactVerificationCommandDto) return <Object?>['reset_verification', command.contactIdHex];
  if (command is BlockContactCommandDto) return <Object?>['block_contact', command.contactIdHex];
  if (command is UnblockContactCommandDto) return <Object?>['unblock_contact', command.contactIdHex];
  if (command is RemoveContactCommandDto) return <Object?>['remove_contact', command.contactIdHex];
  if (command is ClearConversationHistoryCommandDto) return <Object?>['clear_history', command.conversationIdHex];
  if (command is QueueMessageCommandDto) {
    return <Object?>['queue_message', command.conversationIdHex, command.body, command.replyToMessageId ?? ''];
  }
  if (command is RetryMessageCommandDto) return <Object?>['retry_message', command.messageIdHex];
  if (command is MarkConversationReadCommandDto) return <Object?>['mark_read', command.conversationIdHex, command.sendReceipt];
  if (command is QueueAttachmentCommandDto) {
    return <Object?>['queue_attachment', command.conversationIdHex, command.sourcePath, command.name, command.mediaType, command.size];
  }
  if (command is RetryAttachmentCommandDto) return <Object?>['retry_attachment', command.attachmentIdHex];
  if (command is CancelAttachmentCommandDto) return <Object?>['cancel_attachment', command.attachmentIdHex];
  if (command is ExportAttachmentCommandDto) return <Object?>['export_attachment', command.attachmentIdHex, command.destinationPath];
  return null;
}

String _executeNativeCall(List<Object?> call) {
  final bindings = _NativeBindings(ffi.DynamicLibrary.open(_nativeLibraryName()));
  if (bindings.contractVersion() != torcaContractVersion) throw StateError('native contract mismatch');
  final handle = bindings.engineNew();
  if (handle == ffi.nullptr) throw StateError('native process handle unavailable');
  final kind = call[0] as String;
  try {
    switch (kind) {
      case 'create_identity': _callOne(bindings, handle, call[1] as String, bindings.createIdentityIntent);
      case 'create_pairing': bindings.createPairingIntent(handle);
      case 'join_pairing': _callOne(bindings, handle, call[1] as String, bindings.joinPairingIntent);
      case 'approve_pairing': _callOne(bindings, handle, call[1] as String, bindings.approvePairing);
      case 'reject_pairing': _callOne(bindings, handle, call[1] as String, bindings.rejectPairing);
      case 'cancel_pairing': _callOne(bindings, handle, call[1] as String, bindings.cancelPairing);
      case 'rename_contact': _callTwo(bindings, handle, call[1] as String, call[2] as String, bindings.renameContact);
      case 'verify_contact': _callOne(bindings, handle, call[1] as String, bindings.verifyContact);
      case 'reset_verification': _callOne(bindings, handle, call[1] as String, bindings.resetContactVerification);
      case 'block_contact': _callOne(bindings, handle, call[1] as String, bindings.blockContact);
      case 'unblock_contact': _callOne(bindings, handle, call[1] as String, bindings.unblockContact);
      case 'remove_contact': _callOne(bindings, handle, call[1] as String, bindings.removeContact);
      case 'clear_history': _callOne(bindings, handle, call[1] as String, bindings.clearConversationHistory);
      case 'queue_message': _callMessage(bindings, handle, call[1] as String, call[2] as String, call[3] as String);
      case 'retry_message': _callOne(bindings, handle, call[1] as String, bindings.retryMessageIntent);
      case 'mark_read': _callRead(bindings, handle, call[1] as String, call[2] as bool);
      case 'queue_attachment':
        _callAttachment(bindings, handle, call[1] as String, call[2] as String, call[3] as String, call[4] as String, call[5] as int);
      case 'retry_attachment': _callOne(bindings, handle, call[1] as String, bindings.retryAttachment);
      case 'cancel_attachment': _callOne(bindings, handle, call[1] as String, bindings.cancelAttachment);
      case 'export_attachment': _callTwo(bindings, handle, call[1] as String, call[2] as String, bindings.exportAttachment);
      case 'history_page': _callHistoryPage(bindings, handle, call[1] as String, call[2] as int, call[3] as String, call[4] as int);
      case 'history_search': _callHistorySearch(bindings, handle, call[1] as String, call[2] as String, call[3] as int);
      default: throw StateError('unsupported native command');
    }
    if (kind.startsWith('history_')) {
      return _readNativeString(bindings.queryPointer(handle), bindings.queryLength(handle));
    }
    return _readNativeString(bindings.resultPointer(handle), bindings.resultLength(handle));
  } finally {
    bindings.engineDestroy(handle);
  }
}

void _callOne(_NativeBindings bindings, _Handle handle, String value, _OneStringDart operation) {
  final native = _NativeUtf8(bindings, value);
  try { operation(handle, native.pointer, native.length); } finally { native.dispose(); }
}
void _callTwo(_NativeBindings bindings, _Handle handle, String first, String second, _TwoStringsDart operation) {
  final a = _NativeUtf8(bindings, first); final b = _NativeUtf8(bindings, second);
  try { operation(handle, a.pointer, a.length, b.pointer, b.length); } finally { a.dispose(); b.dispose(); }
}
void _callMessage(_NativeBindings bindings, _Handle handle, String conversationId, String body, String replyId) {
  final conversation = _NativeUtf8(bindings, conversationId); final message = _NativeUtf8(bindings, body); final reply = _NativeUtf8(bindings, replyId);
  try { bindings.queueMessageIntent(handle, conversation.pointer, conversation.length, message.pointer, message.length, reply.pointer, reply.length); }
  finally { conversation.dispose(); message.dispose(); reply.dispose(); }
}
void _callRead(_NativeBindings bindings, _Handle handle, String conversationId, bool sendReceipt) {
  final conversation = _NativeUtf8(bindings, conversationId);
  try { bindings.markConversationReadIntent(handle, conversation.pointer, conversation.length, sendReceipt ? 1 : 0); }
  finally { conversation.dispose(); }
}
void _callAttachment(_NativeBindings bindings, _Handle handle, String conversationId, String path, String name, String mediaType, int size) {
  final conversation = _NativeUtf8(bindings, conversationId); final source = _NativeUtf8(bindings, path); final filename = _NativeUtf8(bindings, name); final media = _NativeUtf8(bindings, mediaType);
  try { bindings.queueAttachmentIntent(handle, conversation.pointer, conversation.length, source.pointer, source.length, filename.pointer, filename.length, media.pointer, media.length, size); }
  finally { conversation.dispose(); source.dispose(); filename.dispose(); media.dispose(); }
}
void _callHistoryPage(_NativeBindings bindings, _Handle handle, String conversationId, int beforeAtMs, String beforeId, int limit) {
  final conversation = _NativeUtf8(bindings, conversationId); final before = _NativeUtf8(bindings, beforeId);
  try { bindings.conversationPage(handle, conversation.pointer, conversation.length, beforeAtMs, before.pointer, before.length, limit); }
  finally { conversation.dispose(); before.dispose(); }
}
void _callHistorySearch(_NativeBindings bindings, _Handle handle, String conversationId, String query, int limit) {
  final conversation = _NativeUtf8(bindings, conversationId); final value = _NativeUtf8(bindings, query);
  try { bindings.searchMessages(handle, conversation.pointer, conversation.length, value.pointer, value.length, limit); }
  finally { conversation.dispose(); value.dispose(); }
}

String _nativeLibraryName() {
  if (Platform.isWindows) return 'torca_bridge.dll';
  if (Platform.isAndroid || Platform.isLinux) return 'libtorca_bridge.so';
  if (Platform.isMacOS || Platform.isIOS) return 'libtorca_bridge.dylib';
  throw UnsupportedError('Torca native runtime is unsupported on this platform');
}

String _readNativeString(ffi.Pointer<ffi.Uint8> pointer, int length) {
  if (pointer == ffi.nullptr || length == 0) return '';
  return utf8.decode(pointer.asTypedList(length), allowMalformed: false);
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
        maxAttachmentBytes = library.lookupFunction<_MaxAttachmentNative, _MaxAttachmentDart>('torca_max_attachment_bytes'),
        alloc = library.lookupFunction<_AllocNative, _AllocDart>('torca_alloc'),
        free = library.lookupFunction<_FreeNative, _FreeDart>('torca_free'),
        engineNew = library.lookupFunction<_EngineNewNative, _EngineNewDart>('torca_engine_new'),
        engineDestroy = library.lookupFunction<_EngineDestroyNative, _EngineDestroyDart>('torca_engine_destroy'),
        createIdentityIntent = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_create_identity_intent'),
        createPairingIntent = library.lookupFunction<_NoArgNative, _NoArgDart>('torca_engine_create_pairing_intent'),
        joinPairingIntent = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_join_pairing_intent'),
        approvePairing = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_approve_pairing'),
        rejectPairing = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_reject_pairing'),
        cancelPairing = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_cancel_pairing'),
        renameContact = library.lookupFunction<_TwoStringsNative, _TwoStringsDart>('torca_engine_rename_contact'),
        verifyContact = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_verify_contact'),
        resetContactVerification = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_reset_contact_verification'),
        blockContact = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_block_contact'),
        unblockContact = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_unblock_contact'),
        removeContact = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_remove_contact'),
        clearConversationHistory = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_clear_conversation_history'),
        queueMessageIntent = library.lookupFunction<_MessageIntentNative, _MessageIntentDart>('torca_engine_queue_message_intent'),
        retryMessageIntent = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_retry_message_intent'),
        markConversationReadIntent = library.lookupFunction<_ReadIntentNative, _ReadIntentDart>('torca_engine_mark_conversation_read_intent'),
        queueAttachmentIntent = library.lookupFunction<_AttachmentIntentNative, _AttachmentIntentDart>('torca_engine_queue_attachment_intent'),
        retryAttachment = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_retry_attachment'),
        cancelAttachment = library.lookupFunction<_OneStringNative, _OneStringDart>('torca_engine_cancel_attachment'),
        exportAttachment = library.lookupFunction<_TwoStringsNative, _TwoStringsDart>('torca_engine_export_attachment'),
        conversationPage = library.lookupFunction<_HistoryPageNative, _HistoryPageDart>('torca_engine_conversation_page'),
        searchMessages = library.lookupFunction<_HistorySearchNative, _HistorySearchDart>('torca_engine_search_messages'),
        queryPointer = library.lookupFunction<_PointerNative, _PointerDart>('torca_engine_query_ptr'),
        queryLength = library.lookupFunction<_LengthNative, _LengthDart>('torca_engine_query_len'),
        refreshSnapshot = library.lookupFunction<_RefreshNative, _RefreshDart>('torca_engine_refresh_snapshot'),
        refreshDiagnostics = library.lookupFunction<_RefreshNative, _RefreshDart>('torca_engine_refresh_diagnostics'),
        resultPointer = library.lookupFunction<_PointerNative, _PointerDart>('torca_engine_result_ptr'),
        resultLength = library.lookupFunction<_LengthNative, _LengthDart>('torca_engine_result_len'),
        snapshotPointer = library.lookupFunction<_PointerNative, _PointerDart>('torca_engine_snapshot_ptr'),
        snapshotLength = library.lookupFunction<_LengthNative, _LengthDart>('torca_engine_snapshot_len'),
        diagnosticsPointer = library.lookupFunction<_PointerNative, _PointerDart>('torca_engine_diagnostics_ptr'),
        diagnosticsLength = library.lookupFunction<_LengthNative, _LengthDart>('torca_engine_diagnostics_len');

  final _ContractVersionDart contractVersion;
  final _MaxAttachmentDart maxAttachmentBytes;
  final _AllocDart alloc;
  final _FreeDart free;
  final _EngineNewDart engineNew;
  final _EngineDestroyDart engineDestroy;
  final _OneStringDart createIdentityIntent;
  final _NoArgDart createPairingIntent;
  final _OneStringDart joinPairingIntent;
  final _OneStringDart approvePairing;
  final _OneStringDart rejectPairing;
  final _OneStringDart cancelPairing;
  final _TwoStringsDart renameContact;
  final _OneStringDart verifyContact;
  final _OneStringDart resetContactVerification;
  final _OneStringDart blockContact;
  final _OneStringDart unblockContact;
  final _OneStringDart removeContact;
  final _OneStringDart clearConversationHistory;
  final _MessageIntentDart queueMessageIntent;
  final _OneStringDart retryMessageIntent;
  final _ReadIntentDart markConversationReadIntent;
  final _AttachmentIntentDart queueAttachmentIntent;
  final _OneStringDart retryAttachment;
  final _OneStringDart cancelAttachment;
  final _TwoStringsDart exportAttachment;
  final _HistoryPageDart conversationPage;
  final _HistorySearchDart searchMessages;
  final _PointerDart queryPointer;
  final _LengthDart queryLength;
  final _RefreshDart refreshSnapshot;
  final _RefreshDart refreshDiagnostics;
  final _PointerDart resultPointer;
  final _LengthDart resultLength;
  final _PointerDart snapshotPointer;
  final _LengthDart snapshotLength;
  final _PointerDart diagnosticsPointer;
  final _LengthDart diagnosticsLength;
}
