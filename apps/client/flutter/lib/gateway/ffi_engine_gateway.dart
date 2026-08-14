import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:isolate';

import 'package:flutter/foundation.dart';
import 'package:torca_avatar/torca_avatar.dart';

import '../generated/torca_contract.dart';
import '../platform/native_library.dart';
import 'engine_gateway.dart';

typedef _Handle = ffi.Pointer<ffi.Void>;
typedef _AcquireNative = _Handle Function();
typedef _AcquireDart = _Handle Function();
typedef _ReleaseNative = ffi.Void Function(_Handle);
typedef _ReleaseDart = void Function(_Handle);
typedef _InvokeNative =
    ffi.Int32 Function(
      _Handle,
      ffi.Pointer<ffi.Uint8>,
      ffi.UintPtr,
      ffi.Uint32,
    );
typedef _InvokeDart = int Function(_Handle, ffi.Pointer<ffi.Uint8>, int, int);
typedef _WaitNative =
    ffi.Int32 Function(_Handle, ffi.Uint64, ffi.Uint64, ffi.Uint32);
typedef _WaitDart = int Function(_Handle, int, int, int);
typedef _CancelWaitNative = ffi.Int32 Function(_Handle);
typedef _CancelWaitDart = int Function(_Handle);
typedef _ResponsePtrNative = ffi.Pointer<ffi.Uint8> Function(_Handle);
typedef _ResponsePtrDart = ffi.Pointer<ffi.Uint8> Function(_Handle);
typedef _ResponseLenNative = ffi.UintPtr Function(_Handle);
typedef _ResponseLenDart = int Function(_Handle);
typedef _AllocNative = ffi.Pointer<ffi.Uint8> Function(ffi.UintPtr);
typedef _AllocDart = ffi.Pointer<ffi.Uint8> Function(int);
typedef _FreeNative = ffi.Void Function(ffi.Pointer<ffi.Uint8>, ffi.UintPtr);
typedef _FreeDart = void Function(ffi.Pointer<ffi.Uint8>, int);
typedef _ShutdownNative = ffi.Int32 Function(ffi.Uint32);
typedef _ShutdownDart = int Function(int);
typedef _MetadataPtrNative = ffi.Pointer<ffi.Uint8> Function();
typedef _MetadataPtrDart = ffi.Pointer<ffi.Uint8> Function();
typedef _MetadataLenNative = ffi.UintPtr Function();
typedef _MetadataLenDart = int Function();

void _runtimeWaiterMain(List<Object?> arguments) {
  final ready = arguments[0] as SendPort;
  final handle = _Handle.fromAddress(arguments[1] as int);
  final commandPort = ReceivePort();
  final library = ffi.DynamicLibrary.open(nativeRuntimeLibraryName());
  final wait = library.lookupFunction<_WaitNative, _WaitDart>(
    'torca_runtime_wait_for_revision',
  );
  ready.send(commandPort.sendPort);
  commandPort.listen((message) {
    if (message is! Map) return;
    if (message['stop'] == true) {
      commandPort.close();
      Isolate.exit();
    }
    final reply = message['reply'];
    if (reply is! SendPort) return;
    final result = wait(
      handle,
      message['revision'] as int,
      message['cursor'] as int,
      message['timeoutMs'] as int,
    );
    reply.send(result);
  });
}

class FfiEngineGateway
    implements
        EngineGateway,
        PairingUriParser,
        PairingUriEncoder,
        GatewayAvailability,
        AttachmentCapabilitiesProvider,
        ConversationHistoryProvider,
        BuildInfoProvider,
        RuntimeShutdownGateway,
        AvatarGenomeProvider {
  FfiEngineGateway._(this._worker, this._snapshots, this._eventsController) {
    _worker.events.listen((event) {
      try {
        final value = jsonDecode(event);
        if (value is Map<String, dynamic> && value['eventId'] != null) {
          _eventsController.add(RuntimeEventDto.fromJson(value));
        } else {
          _snapshots.value = _decodeSnapshot(event);
        }
      } on ContractDecodeException catch (error, stackTrace) {
        // Do not silently retain an old, partly compatible snapshot. The
        // stream remains alive, while Diagnostics and the visible runtime
        // error channel receive an actionable contract failure.
        _eventsController.addError(error, stackTrace);
      } on FormatException catch (error, stackTrace) {
        _eventsController.addError(
          ContractDecodeException('Invalid native runtime event: $error'),
          stackTrace,
        );
      } on Object catch (error, stackTrace) {
        _eventsController.addError(
          ContractDecodeException('Invalid native runtime payload: $error'),
          stackTrace,
        );
      }
    });
  }

  static Future<FfiEngineGateway> open() async {
    final worker = await NativeRuntimeWorker.start();
    final gateway = FfiEngineGateway._(
      worker,
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto()),
      StreamController<RuntimeEventDto>.broadcast(),
    );
    AvatarRepository.instance.remoteEnvelopeLoader = (identityId) =>
        gateway.loadAvatarGenome(identityId: identityId);
    return gateway;
  }

  final NativeRuntimeWorker _worker;
  final ValueNotifier<AppSnapshotDto> _snapshots;
  final StreamController<RuntimeEventDto> _eventsController;
  bool _disposed = false;

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Stream<RuntimeEventDto> get events => _eventsController.stream;

  @override
  ClientCapabilitiesDto get capabilities => buildInfo.capabilities;

  @override
  ClientBuildInfo get buildInfo => _worker.buildInfo;

  @override
  bool get isAvailable => !_disposed && _worker.isAlive;

  @override
  String? get failureReason =>
      isAvailable ? null : 'native runtime unavailable';

  Future<BridgeResultDto> initialize() async {
    final response = await _worker.invoke(RuntimeRequestDto.snapshot);
    return _applyResponse(response);
  }

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    if (_disposed) return _unavailable();
    final request = RuntimeRequestDto.command(command);
    if (request == null) {
      return const BridgeResultDto(
        ok: false,
        kind: 'error:contract.operation.unknown',
        error: 'The requested operation is not available.',
      );
    }
    final response = await _worker.invoke(request);
    final result = _applyResponse(response);
    if (command is UpdateProfileCommandDto && result.ok) {
      final profile = _snapshots.value.identity?.displayName;
      if (profile == null || profile.isEmpty) {
        return const BridgeResultDto(
          ok: false,
          kind: 'error:PROFILE_SNAPSHOT_INCONSISTENT',
          error: 'profile.snapshot.inconsistent',
        );
      }
    }
    return result;
  }

  @override
  Future<void> sendLifecycle(String event) async {
    if (_disposed) return;
    await _worker.invoke(RuntimeRequestDto.lifecycle(event));
  }

  @override
  Future<String?> parsePairingUri(String rawUri) async {
    if (_disposed) return null;
    final response = await _worker.invoke(
      RuntimeRequestDto.pairingParse(rawUri),
    );
    final value = jsonDecode(response);
    if (value is! Map<String, dynamic> || value['status'] != 'succeeded') {
      return null;
    }
    final snapshot = value['snapshot'];
    return snapshot is Map ? snapshot['code'] as String? : null;
  }

  @override
  Future<String?> encodePairingUri(String code) async {
    if (_disposed) return null;
    final response = await _worker.invoke(
      RuntimeRequestDto.pairingEncode(code),
    );
    final value = jsonDecode(response);
    if (value is! Map<String, dynamic> || value['status'] != 'succeeded')
      return null;
    final snapshot = value['snapshot'];
    return snapshot is Map ? snapshot['uri'] as String? : null;
  }

  @override
  Future<ConversationPageDto> loadConversationPage(
    String conversationId, {
    MessageDto? before,
    int limit = 100,
  }) async {
    final response = await _worker.invoke(
      RuntimeRequestDto.conversationPage(
        conversationId,
        beforeMessageId: before?.id,
        beforeAtMs: before?.createdAtMs,
        limit: limit.clamp(1, 200),
      ),
    );
    return _decodePage(response);
  }

  @override
  Future<ConversationPageDto> searchConversation(
    String conversationId,
    String query, {
    int limit = 100,
  }) async {
    if (query.trim().isEmpty) {
      return const ConversationPageDto(messages: [], hasMore: false);
    }
    final response = await _worker.invoke(
      RuntimeRequestDto.conversationSearch(
        conversationId,
        query: query,
        limit: limit.clamp(1, 200),
      ),
    );
    return _decodePage(response);
  }

  @override
  Future<String> diagnosticsJson() async {
    final response = await _worker.invoke(RuntimeRequestDto.diagnostics);
    try {
      final value = jsonDecode(response);
      if (value is! Map<String, dynamic> || value['status'] != 'succeeded') {
        throw const ContractDecodeException(
          'Native diagnostics response was not successful',
        );
      }
      final snapshot = value['snapshot'];
      if (snapshot != null && snapshot is! Map<String, dynamic>) {
        throw const ContractDecodeException(
          'Native diagnostics snapshot is not an object',
        );
      }
      return jsonEncode(snapshot ?? const <String, Object?>{'events': []});
    } on ContractDecodeException {
      rethrow;
    } on FormatException catch (error) {
      throw ContractDecodeException('Invalid diagnostics response: $error');
    } on TypeError catch (error) {
      throw ContractDecodeException('Invalid diagnostics field type: $error');
    }
  }

  @override
  Future<AvatarGenomeEnvelope?> loadAvatarGenome({String? identityId}) async {
    if (_disposed) return null;
    final response = await _worker.invoke(
      identityId == null || identityId.isEmpty
          ? RuntimeRequestDto.avatars
          : RuntimeRequestDto.avatarForIdentity(identityId),
    );
    try {
      final value = jsonDecode(response);
      if (value is! Map<String, dynamic> || value['status'] != 'succeeded') {
        return null;
      }
      final raw = value['snapshot'];
      if (raw == null) return null;
      if (raw is! Map<String, dynamic>) {
        throw const ContractDecodeException(
          'Avatar genome response is not an object',
        );
      }
      return AvatarGenomeEnvelope.fromJson(raw);
    } on ContractDecodeException {
      rethrow;
    } on FormatException catch (error) {
      throw ContractDecodeException('Invalid avatar genome response: $error');
    } on TypeError catch (error) {
      throw ContractDecodeException('Invalid avatar genome field type: $error');
    }
  }

  @override
  Future<void> shutdown() async {
    if (_disposed) return;
    await _worker.shutdown();
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    AvatarRepository.instance.remoteEnvelopeLoader = null;
    await _worker.dispose();
    await _eventsController.close();
    _snapshots.dispose();
  }

  BridgeResultDto _applyResponse(String raw) {
    try {
      return _applyResponseUnchecked(raw);
    } on ContractDecodeException {
      return _contractDecodeFailure();
    } on FormatException {
      return _contractDecodeFailure();
    } on TypeError {
      return _contractDecodeFailure();
    }
  }

  BridgeResultDto _applyResponseUnchecked(String raw) {
    final value = jsonDecode(raw);
    if (value is! Map<String, dynamic>) {
      throw const ContractDecodeException('Bridge response is not an object');
    }
    final status = value['status'] as String?;
    final snapshot = value['snapshot'];
    if (snapshot is Map) {
      try {
        _snapshots.value = _decodeSnapshot(jsonEncode(snapshot));
      } on ContractDecodeException {
        return const BridgeResultDto(
          ok: false,
          kind: 'error:contract.decode.failed',
          errorCode: 'CONTRACT_DECODE_FAILED',
          messageKey: 'contract.decode.failed',
          retryable: false,
        );
      }
    }
    if (status == 'succeeded') {
      return BridgeResultDto(
        ok: true,
        kind: value['resultKind'] as String? ?? 'succeeded',
        resourceId: value['resourceId'] as String?,
        inviteUri: value['inviteUri'] as String?,
      );
    }
    final error = value['error'];
    final code = error is Map ? error['code'] as String? : null;
    return BridgeResultDto(
      ok: false,
      kind: 'error:${code ?? 'runtime.operation.failed'}',
      errorCode: code,
      messageKey: error is Map ? error['messageKey'] as String? : null,
      diagnosticId: error is Map ? error['diagnosticId'] as String? : null,
      retryable: error is Map && (error['retryable'] as bool? ?? false),
      resourceId: value['resourceId'] as String?,
      inviteUri: value['inviteUri'] as String?,
    );
  }

  BridgeResultDto _contractDecodeFailure() => const BridgeResultDto(
    ok: false,
    kind: 'error:contract.decode.failed',
    errorCode: 'CONTRACT_DECODE_FAILED',
    messageKey: 'contract.decode.failed',
    retryable: false,
  );

  ConversationPageDto _decodePage(String raw) =>
      decodeConversationPageResponse(raw);

  BridgeResultDto _unavailable() => const BridgeResultDto(
    ok: false,
    kind: 'error:runtime.unavailable',
    error: 'The secure runtime is unavailable.',
  );
}

class NativeRuntimeWorker {
  NativeRuntimeWorker._(
    this._commandPort,
    this._events,
    this._isolate,
    this.buildInfo,
  );

  static Future<NativeRuntimeWorker> start() async {
    final ready = ReceivePort();
    Isolate? isolate;
    ReceivePort? eventPort;
    try {
      isolate = await Isolate.spawn(_workerMain, <Object?>[
        ready.sendPort,
      ], debugName: 'torca-native-runtime-worker');
      final value = await ready.first.timeout(const Duration(seconds: 15));
      if (value is! Map) {
        throw StateError('native runtime worker returned an invalid handshake');
      }
      if (value['error'] != null) {
        throw StateError(value['error'] as String);
      }
      final commandPort = value['commandPort'] as SendPort;
      eventPort = ReceivePort();
      commandPort.send(<String, Object?>{'attachEvents': eventPort.sendPort});
      final metadata = Map<String, dynamic>.from(value['metadata'] as Map);
      ready.close();
      return NativeRuntimeWorker._(
        commandPort,
        eventPort,
        isolate,
        ClientBuildInfo.fromJson(metadata),
      );
    } on Object {
      ready.close();
      eventPort?.close();
      isolate?.kill(priority: Isolate.immediate);
      rethrow;
    }
  }

  final SendPort _commandPort;
  final ReceivePort _events;
  final Isolate _isolate;
  final ClientBuildInfo buildInfo;
  int _requestCounter = 0;
  bool _disposed = false;
  Future<void> _requestTail = Future<void>.value();

  Stream<String> get events =>
      _events.where((value) => value is String).cast<String>();
  bool get isAlive => !_disposed;

  Future<String> invoke(RuntimeRequestDto request) async {
    if (_disposed) throw StateError('native worker disposed');
    final requestId = 'flutter-${++_requestCounter}';
    final queued = _requestTail.then<String>((_) async {
      final first = await _invokeNow(request, requestId);
      if (_isRetryableTimeout(first)) {
        // Reuse the same correlation id. Rust's operation ledger then returns
        // the first committed result instead of duplicating a mutation.
        return _invokeNow(request, requestId);
      }
      return first;
    });
    _requestTail = queued.then<void>(
      (_) {},
      onError: (Object _, StackTrace __) {},
    );
    return queued;
  }

  Future<String> _invokeNow(RuntimeRequestDto request, String requestId) async {
    final reply = ReceivePort();
    try {
      _commandPort.send(<String, Object?>{
        'invoke': request.encode(requestId),
        'timeoutMs': request.timeoutMs,
        'reply': reply.sendPort,
      });
      final value = await reply.first.timeout(
        Duration(milliseconds: request.timeoutMs + 2000),
      );
      return value as String;
    } finally {
      reply.close();
    }
  }

  bool _isRetryableTimeout(String raw) {
    try {
      final value = jsonDecode(raw);
      final error = value is Map ? value['error'] : null;
      return error is Map && error['code'] == 'RUNTIME_TIMEOUT';
    } on Object {
      return false;
    }
  }

  Future<void> shutdown() async {
    if (_disposed) return;
    final queued = _requestTail.then((_) async {
      final reply = ReceivePort();
      try {
        _commandPort.send(<String, Object?>{
          'shutdown': true,
          'reply': reply.sendPort,
        });
        await reply.first.timeout(const Duration(seconds: 17));
      } finally {
        reply.close();
      }
    });
    _requestTail = queued.then<void>(
      (_) {},
      onError: (Object _, StackTrace __) {},
    );
    await queued;
  }

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    try {
      await _requestTail.timeout(const Duration(seconds: 12));
      final reply = ReceivePort();
      try {
        _commandPort.send(<String, Object?>{
          'dispose': true,
          'reply': reply.sendPort,
        });
        await reply.first.timeout(const Duration(seconds: 3));
      } finally {
        reply.close();
      }
    } on Object {
      // A failed isolate cannot acknowledge disposal; killing it is the final
      // bounded cleanup path.
    } finally {
      _events.close();
      _isolate.kill(priority: Isolate.immediate);
    }
  }
}

void _workerMain(List<Object?> arguments) {
  try {
    _workerMainImpl(arguments);
  } on Object catch (error) {
    (arguments[0] as SendPort).send(<String, Object?>{'error': '$error'});
  }
}

void _workerMainImpl(List<Object?> arguments) {
  final ready = arguments[0] as SendPort;
  final commandPort = ReceivePort();
  final library = ffi.DynamicLibrary.open(nativeRuntimeLibraryName());
  final bindings = _WorkerBindings(library);
  final metadata = bindings.metadata();
  if (metadata['nativeAbi'] != torcaNativeAbiVersion ||
      metadata['contractSchema'] != torcaContractVersion) {
    throw StateError('native runtime metadata is incompatible');
  }
  const expectedBuildId = String.fromEnvironment('TORCA_BUILD_ID');
  if (expectedBuildId.isNotEmpty && metadata['buildId'] != expectedBuildId) {
    throw StateError('native runtime build id does not match the application');
  }
  final handle = bindings.acquire();
  if (handle == ffi.nullptr) {
    throw StateError('native runtime could not be acquired');
  }
  SendPort? eventsPort;
  Timer? snapshotTimer;
  final waiterReady = ReceivePort();
  SendPort? waiterPort;
  Isolate? waiterIsolate;
  waiterReady.listen((message) {
    if (message is SendPort) waiterPort = message;
  });
  unawaited(
    Isolate.spawn(_runtimeWaiterMain, <Object?>[
      waiterReady.sendPort,
      handle.address,
    ], debugName: 'torca-runtime-revision-waiter').then<void>((isolate) {
      waiterIsolate = isolate;
    }, onError: (Object _, StackTrace __) {}),
  );
  var notificationCursor = 0;
  var runtimeRevision = 0;
  String? lastSnapshotJson;
  ready.send(<String, Object?>{
    'commandPort': commandPort.sendPort,
    'metadata': metadata,
  });
  commandPort.listen((message) {
    if (message is! Map) return;
    if (message['attachEvents'] is SendPort) {
      eventsPort = message['attachEvents'] as SendPort;
      void pollSnapshot() {
        final target = eventsPort;
        if (target == null) return;
        try {
          final raw = bindings.invoke(
            handle,
            RuntimeRequestDto.runtimePoll(
              notificationCursor,
            ).encode('worker-poll-${DateTime.now().microsecondsSinceEpoch}'),
            5000,
          );
          final decoded = jsonDecode(raw);
          if (decoded is Map && decoded['revision'] is int) {
            final revision = decoded['revision'] as int;
            if (revision > runtimeRevision) runtimeRevision = revision;
          }
          final poll = decoded is Map && decoded['snapshot'] is Map
              ? decoded['snapshot'] as Map
              : const <String, Object?>{};
          final pollSnapshot = poll['snapshot'];
          if (pollSnapshot is Map) {
            final snapshot = pollSnapshot;
            final encoded = jsonEncode(snapshot);
            if (encoded != lastSnapshotJson) {
              lastSnapshotJson = encoded;
              target.send(encoded);
            }
          }
          final events = poll['events'];
          if (events is List) {
            for (final event in events) {
              if (event is Map) {
                final cursor = event['cursor'];
                if (cursor is int && cursor > notificationCursor) {
                  notificationCursor = cursor;
                }
                target.send(jsonEncode(event));
              }
            }
          }
          final afterCursor = poll['afterCursor'];
          if (afterCursor is int && afterCursor > notificationCursor) {
            notificationCursor = afterCursor;
          }
        } on Object {
          // The next scheduled poll retries after a transient native failure.
        }
        final waiter = waiterPort;
        if (waiter == null) {
          snapshotTimer = Timer(const Duration(seconds: 1), pollSnapshot);
          return;
        }
        final reply = ReceivePort();
        waiter.send(<String, Object?>{
          'revision': runtimeRevision,
          'cursor': notificationCursor,
          // Wait indefinitely in native code. Disposal cancels the wait
          // explicitly, so idle state never turns into periodic FFI calls.
          'timeoutMs': 0,
          'reply': reply.sendPort,
        });
        reply.first
            .timeout(const Duration(days: 365))
            .then<void>(
              (_) {
                reply.close();
                snapshotTimer = Timer(Duration.zero, pollSnapshot);
              },
              onError: (Object _, StackTrace __) {
                reply.close();
                snapshotTimer = Timer(const Duration(seconds: 1), pollSnapshot);
              },
            );
      }

      snapshotTimer ??= Timer(Duration.zero, pollSnapshot);
      return;
    }
    if (message['dispose'] == true) {
      snapshotTimer?.cancel();
      bindings.cancelWaitForRevision(handle);
      waiterPort?.send(<String, Object?>{'stop': true});
      waiterReady.close();
      waiterIsolate?.kill(priority: Isolate.immediate);
      bindings.release(handle);
      (message['reply'] as SendPort?)?.send('ok');
      return;
    }
    final reply = message['reply'] as SendPort?;
    if (message['shutdown'] == true) {
      bindings.shutdown(15000);
      reply?.send('ok');
      return;
    }
    final raw = message['invoke'] as String?;
    if (raw == null || reply == null) return;
    final timeoutMs = message['timeoutMs'] as int? ?? 10000;
    try {
      final result = bindings.invoke(handle, raw, timeoutMs);
      reply.send(result);
    } on Object catch (error) {
      final requestId =
          (jsonDecode(raw) as Map?)?['requestId'] as String? ?? '';
      reply.send(
        jsonEncode(<String, Object?>{
          'schema': 1,
          'requestId': requestId,
          'status': 'failed',
          'resultKind': 'error',
          'snapshot': null,
          'error': <String, Object?>{
            'code': 'RUNTIME_TIMEOUT',
            'category': 'runtime',
            'severity': 'error',
            'retryable': true,
            'messageKey': 'runtime.timeout',
            'diagnosticId': '$error',
          },
        }),
      );
    }
  });
}

class _WorkerBindings {
  _WorkerBindings(ffi.DynamicLibrary library)
    : _acquire = library.lookupFunction<_AcquireNative, _AcquireDart>(
        'torca_runtime_acquire',
      ),
      _release = library.lookupFunction<_ReleaseNative, _ReleaseDart>(
        'torca_runtime_release',
      ),
      _invoke = library.lookupFunction<_InvokeNative, _InvokeDart>(
        'torca_runtime_invoke',
      ),
      _waitForRevision = library.lookupFunction<_WaitNative, _WaitDart>(
        'torca_runtime_wait_for_revision',
      ),
      _cancelWaitForRevision = library
          .lookupFunction<_CancelWaitNative, _CancelWaitDart>(
            'torca_runtime_cancel_revision_wait',
          ),
      _responsePtr = library
          .lookupFunction<_ResponsePtrNative, _ResponsePtrDart>(
            'torca_runtime_response_ptr',
          ),
      _responseLen = library
          .lookupFunction<_ResponseLenNative, _ResponseLenDart>(
            'torca_runtime_response_len',
          ),
      _alloc = library.lookupFunction<_AllocNative, _AllocDart>('torca_alloc'),
      _free = library.lookupFunction<_FreeNative, _FreeDart>('torca_free'),
      _shutdown = library.lookupFunction<_ShutdownNative, _ShutdownDart>(
        'torca_runtime_shutdown',
      ),
      _metadataPtr = library
          .lookupFunction<_MetadataPtrNative, _MetadataPtrDart>(
            'torca_runtime_metadata_ptr',
          ),
      _metadataLen = library
          .lookupFunction<_MetadataLenNative, _MetadataLenDart>(
            'torca_runtime_metadata_len',
          );

  final _AcquireDart _acquire;
  final _ReleaseDart _release;
  final _InvokeDart _invoke;
  final _WaitDart _waitForRevision;
  final _CancelWaitDart _cancelWaitForRevision;
  final _ResponsePtrDart _responsePtr;
  final _ResponseLenDart _responseLen;
  final _AllocDart _alloc;
  final _FreeDart _free;
  final _ShutdownDart _shutdown;
  final _MetadataPtrDart _metadataPtr;
  final _MetadataLenDart _metadataLen;

  _Handle acquire() => _acquire();
  void release(_Handle handle) => _release(handle);
  int shutdown(int timeoutMs) => _shutdown(timeoutMs);

  int waitForRevision(
    _Handle handle,
    int afterRevision,
    int afterCursor,
    int timeoutMs,
  ) => _waitForRevision(handle, afterRevision, afterCursor, timeoutMs);

  int cancelWaitForRevision(_Handle handle) => _cancelWaitForRevision(handle);

  Map<String, dynamic> metadata() {
    final pointer = _metadataPtr();
    final length = _metadataLen();
    if (pointer == ffi.nullptr || length == 0) {
      throw StateError('native runtime metadata is empty');
    }
    final value = jsonDecode(utf8.decode(pointer.asTypedList(length)));
    if (value is! Map<String, dynamic>) {
      throw StateError('native runtime metadata is invalid');
    }
    return value;
  }

  String invoke(_Handle handle, String request, int timeoutMs) {
    final bytes = utf8.encode(request);
    final pointer = _alloc(bytes.length);
    if (pointer == ffi.nullptr)
      throw StateError('native request allocation failed');
    pointer.asTypedList(bytes.length).setAll(0, bytes);
    try {
      final status = _invoke(handle, pointer, bytes.length, timeoutMs);
      if (status != 0) throw StateError('native invoke failed: $status');
      final resultPointer = _responsePtr(handle);
      final resultLength = _responseLen(handle);
      if (resultPointer == ffi.nullptr || resultLength == 0) {
        throw StateError('native response is empty');
      }
      return utf8.decode(resultPointer.asTypedList(resultLength));
    } finally {
      _free(pointer, bytes.length);
    }
  }
}

AppSnapshotDto _decodeSnapshot(String raw) {
  try {
    final value = jsonDecode(raw);
    if (value is! Map<String, dynamic>) {
      throw const ContractDecodeException('Snapshot is not a contract object');
    }
    return AppSnapshotDto.fromJson(value);
  } on ContractDecodeException {
    rethrow;
  } on FormatException catch (error) {
    throw ContractDecodeException('Invalid snapshot: ${error.message}');
  } on TypeError catch (error) {
    throw ContractDecodeException('Invalid snapshot field type: $error');
  }
}

@visibleForTesting
ConversationPageDto decodeConversationPageResponse(String raw) {
  final value = jsonDecode(raw);
  if (value is! Map<String, dynamic>) {
    throw const FormatException('Conversation response is not an object');
  }
  if (value['status'] == 'failed') {
    final error = value['error'];
    final message = error is Map<String, dynamic>
        ? error['messageKey'] as String?
        : null;
    throw StateError(message ?? 'Conversation history query failed');
  }
  final snapshot = value['snapshot'];
  final page = snapshot is Map<String, dynamic> ? snapshot : value;
  final messages =
      (page['messages'] is List<Object?>
              ? page['messages'] as List<Object?>
              : const <Object?>[])
          .whereType<Map<String, dynamic>>()
          .map(MessageDto.fromJson)
          .toList(growable: false);
  return ConversationPageDto(
    messages: messages,
    hasMore: page['hasMore'] == true,
  );
}
