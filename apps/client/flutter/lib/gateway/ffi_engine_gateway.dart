import 'dart:async';
import 'dart:convert';
import 'dart:developer' as developer;
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
        _logFfi('CONTRACT_DECODE_FAILED', <String, Object?>{'error': '$error'});
        _eventsController.addError(error, stackTrace);
      } on FormatException catch (error, stackTrace) {
        _logFfi('EVENT_DECODE_FAILED', <String, Object?>{'error': '$error'});
        _eventsController.addError(
          ContractDecodeException('Invalid native runtime event: $error'),
          stackTrace,
        );
      } on Object catch (error, stackTrace) {
        _logFfi('EVENT_DECODE_FAILED', <String, Object?>{'error': '$error'});
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
    return _decodeDiagnosticsQuery(
      await _worker.invoke(RuntimeRequestDto.diagnostics),
      responseName: 'Native diagnostics response',
      fallbackSnapshot: const <String, Object?>{'events': []},
    );
  }

  String _decodeDiagnosticsQuery(
    String response, {
    required String responseName,
    required Map<String, Object?> fallbackSnapshot,
  }) {
    try {
      final value = jsonDecode(response);
      if (value is! Map<String, dynamic> || value['status'] != 'succeeded') {
        throw ContractDecodeException('$responseName was not successful');
      }
      final snapshot = value['snapshot'];
      if (snapshot != null && snapshot is! Map<String, dynamic>) {
        throw ContractDecodeException(
          '$responseName snapshot is not an object',
        );
      }
      return jsonEncode(snapshot ?? fallbackSnapshot);
    } on ContractDecodeException {
      rethrow;
    } on FormatException catch (error) {
      throw ContractDecodeException('Invalid $responseName: $error');
    } on TypeError catch (error) {
      throw ContractDecodeException('Invalid $responseName field type: $error');
    }
  }

  @override
  Future<String> diagnosticsLogTailsJson() async => _decodeDiagnosticsQuery(
    await _worker.invoke(RuntimeRequestDto.diagnosticsLogTails),
    responseName: 'Native diagnostics log tail response',
    fallbackSnapshot: const <String, Object?>{'logs': []},
  );

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
    } on ContractDecodeException catch (error) {
      return _contractDecodeFailure(error);
    } on FormatException catch (error) {
      return _contractDecodeFailure(error);
    } on TypeError catch (error) {
      return _contractDecodeFailure(error);
    }
  }

  BridgeResultDto _applyResponseUnchecked(String raw) {
    final value = jsonDecode(raw);
    if (value is! Map<String, dynamic>) {
      throw const ContractDecodeException('Bridge response is not an object');
    }
    final status = value['status'] as String?;
    final snapshot = value['snapshot'];
    final resultKind = value['resultKind'] as String?;
    final resourceId = value['resourceId'] as String?;
    final error = value['error'];
    _logFfi('RESPONSE', <String, Object?>{
      'status': status,
      'resultKind': resultKind,
      'resourceId': resourceId,
      'inviteUriPresent':
          value['inviteUri'] is String &&
          (value['inviteUri'] as String).isNotEmpty,
      'snapshotPresent': snapshot is Map,
      'snapshotPairings': snapshot is Map && snapshot['pairings'] is List
          ? (snapshot['pairings'] as List).length
          : null,
      'errorCode': error is Map ? error['code'] : null,
    });
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
        kind: resultKind ?? 'succeeded',
        resourceId: resourceId,
        inviteUri: value['inviteUri'] as String?,
      );
    }
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

  BridgeResultDto _contractDecodeFailure([Object? cause]) => BridgeResultDto(
    ok: false,
    kind: 'error:contract.decode.failed',
    error: cause == null ? 'native response contract decode failed' : '$cause',
    errorCode: 'CONTRACT_DECODE_FAILED',
    messageKey: 'contract.decode.failed',
    retryable: false,
  );

  void _logFfi(String code, Map<String, Object?> context) {
    // Keep this structured and metadata-only: invitation URIs, message text,
    // fingerprints and attachment paths must never be emitted to logcat.
    developer.log(
      jsonEncode(<String, Object?>{
        'schema': 1,
        'domain': 'ffi',
        'code': code,
        'context': context,
      }),
      name: 'torca.ffi',
    );
  }

  ConversationPageDto _decodePage(String raw) =>
      decodeConversationPageResponse(raw);

  BridgeResultDto _unavailable() => const BridgeResultDto(
    ok: false,
    kind: 'error:runtime.unavailable',
    error: 'The secure runtime is unavailable.',
  );
}

class _QueuedRuntimeRequest {
  _QueuedRuntimeRequest(this.request, this.requestId, this.completer);

  final RuntimeRequestDto request;
  final String requestId;
  final Completer<String> completer;
}

class NativeRuntimeWorker {
  NativeRuntimeWorker._(
    this._commandPort,
    this._events,
    this._isolate,
    this._exitPort,
    this.buildInfo,
  ) {
    _exitPort.listen((_) {
      _dead = true;
      _failQueued(StateError('native runtime worker exited'));
      _exitPort.close();
    });
  }

  static Future<NativeRuntimeWorker> start() async {
    final ready = ReceivePort();
    Isolate? isolate;
    ReceivePort? eventPort;
    ReceivePort? exitPort;
    try {
      isolate = await Isolate.spawn(_workerMain, <Object?>[
        ready.sendPort,
      ], debugName: 'torca-native-runtime-worker');
      exitPort = ReceivePort();
      isolate.addOnExitListener(exitPort.sendPort);
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
        exitPort,
        ClientBuildInfo.fromJson(metadata),
      );
    } on Object {
      ready.close();
      eventPort?.close();
      exitPort?.close();
      isolate?.kill(priority: Isolate.immediate);
      rethrow;
    }
  }

  final SendPort _commandPort;
  final ReceivePort _events;
  final Isolate _isolate;
  final ReceivePort _exitPort;
  final ClientBuildInfo buildInfo;
  int _requestCounter = 0;
  bool _disposed = false;
  bool _dead = false;
  final List<_QueuedRuntimeRequest> _interactiveQueue =
      <_QueuedRuntimeRequest>[];
  final List<_QueuedRuntimeRequest> _queryQueue = <_QueuedRuntimeRequest>[];
  bool _pumping = false;

  Stream<String> get events =>
      _events.where((value) => value is String).cast<String>();

  /// True only while the Dart worker isolate has not exited. This is more
  /// useful to the UI than checking whether the wrapper object was disposed;
  /// an isolate can die after a native panic or channel failure.
  bool get isAlive => !_disposed && !_dead;

  Future<String> invoke(RuntimeRequestDto request) async {
    if (_disposed) throw StateError('native worker disposed');
    if (_dead) throw StateError('native runtime worker exited');
    final requestId = 'flutter-${++_requestCounter}';
    final completer = Completer<String>();
    final queued = _QueuedRuntimeRequest(request, requestId, completer);
    (request.kind == 'query' ? _queryQueue : _interactiveQueue).add(queued);
    unawaited(_pumpRequests());
    return completer.future;
  }

  /// A single native handle still has one response buffer, so invocations are
  /// executed serially. Interactive commands are nevertheless kept in a
  /// separate priority lane: a stale history/diagnostics query can no longer
  /// starve pairing, Radio, lifecycle or message commands in the Dart queue.
  Future<void> _pumpRequests() async {
    if (_pumping) return;
    _pumping = true;
    try {
      while (!_disposed &&
          (_interactiveQueue.isNotEmpty || _queryQueue.isNotEmpty)) {
        final queued = _interactiveQueue.isNotEmpty
            ? _interactiveQueue.removeAt(0)
            : _queryQueue.removeAt(0);
        try {
          var response = await _invokeNow(queued.request, queued.requestId);
          if (queued.request.kind != 'query' && _isRetryableTimeout(response)) {
            // Reuse the same correlation id. Rust's operation ledger then
            // returns the first committed result instead of duplicating a
            // mutation.
            response = await _invokeNow(queued.request, queued.requestId);
          }
          if (!queued.completer.isCompleted)
            queued.completer.complete(response);
        } on Object catch (error, stackTrace) {
          if (!queued.completer.isCompleted) {
            queued.completer.completeError(error, stackTrace);
          }
        }
      }
    } finally {
      _pumping = false;
      if (!_disposed &&
          (_interactiveQueue.isNotEmpty || _queryQueue.isNotEmpty)) {
        unawaited(_pumpRequests());
      }
    }
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
    while (_pumping || _interactiveQueue.isNotEmpty || _queryQueue.isNotEmpty) {
      await Future<void>.delayed(const Duration(milliseconds: 1));
    }
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
  }

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    final error = StateError('native worker disposed');
    _failQueued(error);
    await _disposeNative();
  }

  void _failQueued(Object error) {
    for (final queued in <_QueuedRuntimeRequest>[
      ..._interactiveQueue,
      ..._queryQueue,
    ]) {
      if (!queued.completer.isCompleted) queued.completer.completeError(error);
    }
    _interactiveQueue.clear();
    _queryQueue.clear();
  }

  Future<void> _disposeNative() async {
    try {
      final deadline = DateTime.now().add(const Duration(seconds: 12));
      while (_pumping && DateTime.now().isBefore(deadline)) {
        await Future<void>.delayed(const Duration(milliseconds: 1));
      }
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
      _exitPort.close();
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
      metadata['contractSchema'] != torcaContractVersion ||
      metadata['storageEpoch'] != torcaStorageEpoch) {
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
  var fallbackFailures = 0;
  var fallbackPolls = 0;
  var waiterWakeups = 0;
  var snapshotDecodes = 0;
  var snapshotChanges = 0;
  var consecutiveImmediateWakeups = 0;
  DateTime? lastSnapshotSentAt;
  var fallbackDegradedReported = false;
  var disposed = false;
  Duration fallbackDelay() {
    const seconds = <int>[1, 2, 5, 15, 30];
    final index = fallbackFailures.clamp(0, seconds.length - 1);
    fallbackFailures++;
    fallbackPolls++;
    if (fallbackFailures >= seconds.length && !fallbackDegradedReported) {
      fallbackDegradedReported = true;
      developer.log(
        'revision waiter degraded; using bounded fallback polling',
        name: 'torca.runtime',
        error: <String, Object?>{
          'fallbackPolls': fallbackPolls,
          'waiterWakeups': waiterWakeups,
          'snapshotDecodes': snapshotDecodes,
          'snapshotChanges': snapshotChanges,
        },
      );
    }
    return Duration(seconds: seconds[index]);
  }

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
        if (disposed) return;
        final target = eventsPort;
        if (target == null) return;
        try {
          final raw = bindings.invoke(
            handle,
            RuntimeRequestDto.runtimePoll(
              notificationCursor,
              afterRevision: runtimeRevision,
            ).encode('worker-poll-${DateTime.now().microsecondsSinceEpoch}'),
            5000,
          );
          snapshotDecodes++;
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
            final changed = encoded != lastSnapshotJson;
            if (changed) {
              lastSnapshotJson = encoded;
              snapshotChanges++;
            }
            // Idle runtimes do not need to cross the isolate boundary on
            // every observation. Keep a sparse heartbeat for freshness, but
            // only publish a full snapshot when it changed. This also makes
            // a revision flood observable without multiplying JSON work.
            final now = DateTime.now();
            final shouldEmit = changed || lastSnapshotSentAt == null ||
                now.difference(lastSnapshotSentAt!) >=
                    const Duration(seconds: 5);
            if (shouldEmit) {
              target.send(encoded);
              lastSnapshotSentAt = now;
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
          snapshotTimer = Timer(fallbackDelay(), pollSnapshot);
          return;
        }
        final reply = ReceivePort();
        waiter.send(<String, Object?>{
          'revision': runtimeRevision,
          'cursor': notificationCursor,
          // Five seconds is only a safety heartbeat. Provider changes still
          // wake the waiter immediately, while unchanged snapshots stay in
          // the native/worker boundary instead of being rebuilt every 2s.
          'timeoutMs': 5000,
          'reply': reply.sendPort,
        });
        reply.first
            .timeout(const Duration(days: 365))
            .then<void>(
              (value) {
                reply.close();
                if (disposed) return;
                final waitResult = value is int ? value : -1;
                if (waitResult == 1 || waitResult == 0) {
                  fallbackFailures = 0;
                  if (waitResult == 1) {
                    waiterWakeups++;
                    consecutiveImmediateWakeups++;
                  } else {
                    consecutiveImmediateWakeups = 0;
                  }
                  fallbackDegradedReported = false;
                  final delay = consecutiveImmediateWakeups > 10
                      ? const Duration(milliseconds: 100)
                      : consecutiveImmediateWakeups > 3
                          ? const Duration(milliseconds: 10)
                          : Duration.zero;
                  snapshotTimer = Timer(delay, pollSnapshot);
                } else {
                  snapshotTimer = Timer(fallbackDelay(), pollSnapshot);
                }
              },
              onError: (Object _, StackTrace __) {
                reply.close();
                if (disposed) return;
                snapshotTimer = Timer(fallbackDelay(), pollSnapshot);
              },
            );
      }

      snapshotTimer ??= Timer(Duration.zero, pollSnapshot);
      return;
    }
    if (message['dispose'] == true) {
      disposed = true;
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
