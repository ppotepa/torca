import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';
import 'engine_gateway.dart';
import 'ffi_engine_gateway_v4.dart' as base;

typedef _Handle = ffi.Pointer<ffi.Void>;
typedef _EngineNewNative = _Handle Function();
typedef _EngineNewDart = _Handle Function();
typedef _EngineDestroyNative = ffi.Void Function(_Handle);
typedef _EngineDestroyDart = void Function(_Handle);
typedef _AllocNative = ffi.Pointer<ffi.Uint8> Function(ffi.UintPtr);
typedef _AllocDart = ffi.Pointer<ffi.Uint8> Function(int);
typedef _FreeNative = ffi.Void Function(ffi.Pointer<ffi.Uint8>, ffi.UintPtr);
typedef _FreeDart = void Function(ffi.Pointer<ffi.Uint8>, int);
typedef _IdNative = ffi.Int32 Function(_Handle, ffi.Pointer<ffi.Uint8>, ffi.UintPtr);
typedef _IdDart = int Function(_Handle, ffi.Pointer<ffi.Uint8>, int);
typedef _QueueMessageReplyNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Int64,
);
typedef _QueueMessageReplyDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  int,
);
typedef _QueueAttachmentNative = ffi.Int32 Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Pointer<ffi.Uint8>, ffi.UintPtr,
  ffi.Uint64,
);
typedef _QueueAttachmentDart = int Function(
  _Handle,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  ffi.Pointer<ffi.Uint8>, int,
  int,
);
typedef _RefreshNative = ffi.Int32 Function(_Handle);
typedef _RefreshDart = int Function(_Handle);
typedef _PtrNative = ffi.Pointer<ffi.Uint8> Function(_Handle);
typedef _PtrDart = ffi.Pointer<ffi.Uint8> Function(_Handle);
typedef _LenNative = ffi.UintPtr Function(_Handle);
typedef _LenDart = int Function(_Handle);

class FfiEngineGateway implements EngineGateway {
  FfiEngineGateway._(this._base, this._bindings, this._handle) {
    _base.snapshots.addListener(_baseChanged);
  }

  static FfiEngineGateway open({ffi.DynamicLibrary? library}) {
    final lib = library ?? ffi.DynamicLibrary.open(_libraryName());
    final wrapped = base.FfiEngineGateway.open(library: lib);
    final bindings = _AttachmentBindings(lib);
    final handle = bindings.engineNew();
    if (handle == ffi.nullptr) {
      throw StateError('native Torca process handle could not be acquired');
    }
    return FfiEngineGateway._(wrapped, bindings, handle);
  }

  static String _libraryName() {
    if (Platform.isWindows) return 'torca_bridge.dll';
    if (Platform.isAndroid || Platform.isLinux) return 'libtorca_bridge.so';
    if (Platform.isMacOS || Platform.isIOS) return 'libtorca_bridge.dylib';
    throw UnsupportedError('Torca native runtime is unsupported on this platform');
  }

  final base.FfiEngineGateway _base;
  final _AttachmentBindings _bindings;
  final _Handle _handle;
  final ValueNotifier<AppSnapshotDto> _snapshots = ValueNotifier(const AppSnapshotDto());
  bool _disposed = false;

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  Future<BridgeResultDto> initialize() async {
    final result = await _base.initialize();
    if (result.ok) _refreshFullSnapshot();
    return result;
  }

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    if (_disposed) return const BridgeResultDto(ok: false, kind: 'error', error: 'native engine gateway is disposed');
    if (command is QueueMessageCommandDto && command.replyToMessageId != null) {
      return _queueMessageReply(command);
    }
    if (command is QueueAttachmentCommandDto) return _queueAttachment(command);
    if (command is RetryAttachmentCommandDto) return _idAttachment(command.attachmentIdHex, _bindings.retryAttachment);
    if (command is CancelAttachmentCommandDto) return _idAttachment(command.attachmentIdHex, _bindings.cancelAttachment);
    final result = await _base.execute(command);
    if (result.ok) _refreshFullSnapshot();
    return result;
  }

  @override
  Future<String> diagnosticsJson() => _base.diagnosticsJson();

  Future<BridgeResultDto> _queueMessageReply(QueueMessageCommandDto command) async {
    final replyId = command.replyToMessageId;
    if (replyId == null || replyId.isEmpty) {
      return const BridgeResultDto(ok: false, kind: 'error', error: 'reply message id is required');
    }
    final message = _Utf8(_bindings, command.messageIdHex);
    final conversation = _Utf8(_bindings, command.conversationIdHex);
    final body = _Utf8(_bindings, command.body);
    final reply = _Utf8(_bindings, replyId);
    try {
      _bindings.queueMessageReply(
        _handle,
        message.pointer, message.length,
        conversation.pointer, conversation.length,
        body.pointer, body.length,
        reply.pointer, reply.length,
        command.atMs,
      );
    } finally {
      message.dispose();
      conversation.dispose();
      body.dispose();
      reply.dispose();
    }
    final result = _result();
    if (result.ok) _refreshFullSnapshot();
    return result;
  }

  Future<BridgeResultDto> _queueAttachment(QueueAttachmentCommandDto command) async {
    final attachment = _Utf8(_bindings, command.attachmentIdHex);
    final message = _Utf8(_bindings, command.messageIdHex);
    final conversation = _Utf8(_bindings, command.conversationIdHex);
    final path = _Utf8(_bindings, command.sourcePath);
    final name = _Utf8(_bindings, command.name);
    final media = _Utf8(_bindings, command.mediaType);
    try {
      _bindings.queueAttachment(
        _handle,
        attachment.pointer, attachment.length,
        message.pointer, message.length,
        conversation.pointer, conversation.length,
        path.pointer, path.length,
        name.pointer, name.length,
        media.pointer, media.length,
        command.size,
      );
    } finally {
      attachment.dispose(); message.dispose(); conversation.dispose();
      path.dispose(); name.dispose(); media.dispose();
    }
    final result = _result();
    if (result.ok) _refreshFullSnapshot();
    return result;
  }

  Future<BridgeResultDto> _idAttachment(String id, _IdDart call) async {
    final value = _Utf8(_bindings, id);
    try { call(_handle, value.pointer, value.length); } finally { value.dispose(); }
    final result = _result();
    if (result.ok) _refreshFullSnapshot();
    return result;
  }

  void _baseChanged() {
    if (!_disposed) _refreshFullSnapshot();
  }

  void _refreshFullSnapshot() {
    if (_bindings.refreshSnapshot(_handle) != 0) return;
    final raw = _read(_bindings.snapshotPointer(_handle), _bindings.snapshotLength(_handle));
    if (raw.isEmpty) return;
    _snapshots.value = _decodeSnapshot(raw);
  }

  BridgeResultDto _result() {
    final raw = _read(_bindings.resultPointer(_handle), _bindings.resultLength(_handle));
    final map = _map(jsonDecode(raw));
    return BridgeResultDto(
      ok: map['ok'] == true,
      kind: map['kind'] as String? ?? 'error',
      error: map['error'] as String?,
    );
  }

  AppSnapshotDto _decodeSnapshot(String raw) {
    final map = _map(jsonDecode(raw));
    if (map['contractVersion'] != torcaContractVersion) throw const FormatException('native contract mismatch');
    final identityMap = map['identity'] == null ? null : _map(map['identity']);
    return AppSnapshotDto(
      identity: identityMap == null ? null : IdentityDto(displayName: identityMap['displayName'] as String),
      torState: map['torState'] as String? ?? 'stopped',
      onionAddress: map['onionAddress'] as String?,
      pairings: _items(map['pairings']).map((value) {
        final item = _map(value);
        return PairingDto(
          id: item['id'] as String, code: item['code'] as String, role: item['role'] as String,
          state: item['state'] as String, expiresAtMs: item['expiresAtMs'] as int,
          localApproved: item['localApproved'] as bool, remoteApproved: item['remoteApproved'] as bool,
        );
      }).toList(growable: false),
      contacts: _items(map['contacts']).map((value) {
        final item = _map(value);
        return ContactDto(
          id: item['id'] as String, onionAddress: item['onionAddress'] as String,
          status: item['status'] as String, connectionState: item['connectionState'] as String,
        );
      }).toList(growable: false),
      conversations: _items(map['conversations']).map((value) {
        final item = _map(value);
        return ConversationDto(id: item['id'] as String, contactId: item['contactId'] as String, status: item['status'] as String);
      }).toList(growable: false),
      messages: _items(map['messages']).map((value) {
        final item = _map(value);
        return MessageDto(
          id: item['id'] as String, conversationId: item['conversationId'] as String,
          body: item['body'] as String, direction: item['direction'] as String, status: item['status'] as String,
          replyToMessageId: item['replyToMessageId'] as String?,
        );
      }).toList(growable: false),
      attachments: _items(map['attachments']).map((value) {
        final item = _map(value);
        return AttachmentDto(
          id: item['id'] as String, messageId: item['messageId'] as String,
          name: item['name'] as String, mediaType: item['mediaType'] as String,
          size: item['size'] as int, status: item['status'] as String, offset: item['offset'] as int,
        );
      }).toList(growable: false),
    );
  }

  String _read(ffi.Pointer<ffi.Uint8> pointer, int length) {
    if (pointer == ffi.nullptr || length == 0) return '';
    return utf8.decode(pointer.asTypedList(length), allowMalformed: false);
  }
  Map<String, Object?> _map(Object? value) => (value as Map).map((key, value) => MapEntry(key.toString(), value));
  List<Object?> _items(Object? value) => value is List ? value.cast<Object?>() : const [];

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _base.snapshots.removeListener(_baseChanged);
    await _base.dispose();
    _bindings.engineDestroy(_handle);
    _snapshots.dispose();
  }
}

class _Utf8 {
  _Utf8(this.bindings, String value) : bytes = utf8.encode(value) {
    pointer = bindings.alloc(bytes.length);
    if (bytes.isNotEmpty) pointer.asTypedList(bytes.length).setAll(0, bytes);
  }
  final _AttachmentBindings bindings;
  final List<int> bytes;
  late final ffi.Pointer<ffi.Uint8> pointer;
  int get length => bytes.length;
  void dispose() => bindings.free(pointer, bytes.length);
}

class _AttachmentBindings {
  _AttachmentBindings(ffi.DynamicLibrary library)
      : engineNew = library.lookupFunction<_EngineNewNative, _EngineNewDart>('torca_engine_new'),
        engineDestroy = library.lookupFunction<_EngineDestroyNative, _EngineDestroyDart>('torca_engine_destroy'),
        alloc = library.lookupFunction<_AllocNative, _AllocDart>('torca_alloc'),
        free = library.lookupFunction<_FreeNative, _FreeDart>('torca_free'),
        queueMessageReply = library.lookupFunction<_QueueMessageReplyNative, _QueueMessageReplyDart>('torca_engine_queue_message_reply'),
        queueAttachment = library.lookupFunction<_QueueAttachmentNative, _QueueAttachmentDart>('torca_engine_queue_attachment'),
        retryAttachment = library.lookupFunction<_IdNative, _IdDart>('torca_engine_retry_attachment'),
        cancelAttachment = library.lookupFunction<_IdNative, _IdDart>('torca_engine_cancel_attachment'),
        refreshSnapshot = library.lookupFunction<_RefreshNative, _RefreshDart>('torca_engine_refresh_snapshot'),
        resultPointer = library.lookupFunction<_PtrNative, _PtrDart>('torca_engine_result_ptr'),
        resultLength = library.lookupFunction<_LenNative, _LenDart>('torca_engine_result_len'),
        snapshotPointer = library.lookupFunction<_PtrNative, _PtrDart>('torca_engine_snapshot_ptr'),
        snapshotLength = library.lookupFunction<_LenNative, _LenDart>('torca_engine_snapshot_len');
  final _EngineNewDart engineNew;
  final _EngineDestroyDart engineDestroy;
  final _AllocDart alloc;
  final _FreeDart free;
  final _QueueMessageReplyDart queueMessageReply;
  final _QueueAttachmentDart queueAttachment;
  final _IdDart retryAttachment;
  final _IdDart cancelAttachment;
  final _RefreshDart refreshSnapshot;
  final _PtrDart resultPointer;
  final _LenDart resultLength;
  final _PtrDart snapshotPointer;
  final _LenDart snapshotLength;
}