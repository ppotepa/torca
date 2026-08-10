import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:isolate';

import 'package:flutter/foundation.dart';

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

class FfiEngineGateway
    implements
        EngineGateway,
        PairingUriParser,
        GatewayAvailability,
        AttachmentCapabilitiesProvider,
        ConversationHistoryProvider,
        RuntimeShutdownGateway {
  FfiEngineGateway._(this._worker, this._snapshots, this._eventsController) {
    _worker.events.listen((event) {
      final value = jsonDecode(event);
      if (value is Map<String, dynamic> && value['eventId'] != null) {
        _eventsController.add(RuntimeEventDto.fromJson(value));
      } else {
        final snapshot = _decodeSnapshot(event);
        if (snapshot != null) _snapshots.value = snapshot;
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
    await gateway.initialize();
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
  AppCapabilities get capabilities =>
      const AppCapabilities(maxAttachmentBytes: 16 * 1024 * 1024);

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
  Future<ConversationPageDto> loadConversationPage(
    String conversationId, {
    MessageDto? before,
    int limit = 100,
  }) async {
    final response = await _worker.invoke(
      RuntimeRequestDto.conversationPage(
        conversationId,
        beforeMessageId: before?.id,
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
  Future<String> diagnosticsJson() async => '{"events":[]}';

  @override
  Future<void> shutdown() async {
    if (_disposed) return;
    await _worker.shutdown();
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _worker.dispose();
    await _eventsController.close();
    _snapshots.dispose();
  }

  BridgeResultDto _applyResponse(String raw) {
    final value = jsonDecode(raw);
    if (value is! Map<String, dynamic>) return _unavailable();
    final status = value['status'] as String?;
    final snapshot = value['snapshot'];
    if (snapshot is Map) {
      final decoded = _decodeSnapshot(jsonEncode(snapshot));
      if (decoded != null) _snapshots.value = decoded;
    }
    if (status == 'succeeded') {
      return BridgeResultDto(
        ok: true,
        kind: value['resultKind'] as String? ?? 'succeeded',
      );
    }
    final error = value['error'];
    final code = error is Map ? error['code'] as String? : null;
    return BridgeResultDto(
      ok: false,
      kind: 'error:${code ?? 'runtime.operation.failed'}',
      error: error is Map ? error['messageKey'] as String? : null,
    );
  }

  ConversationPageDto _decodePage(String raw) {
    final value = jsonDecode(raw);
    if (value is! Map<String, dynamic>) {
      return const ConversationPageDto(messages: [], hasMore: false);
    }
    final messages =
        (value['messages'] is List<Object?>
                ? value['messages'] as List<Object?>
                : const <Object?>[])
            .whereType<Map<String, dynamic>>()
            .map(_decodeMessage)
            .toList(growable: false);
    return ConversationPageDto(
      messages: messages,
      hasMore: value['hasMore'] == true,
    );
  }

  BridgeResultDto _unavailable() => const BridgeResultDto(
    ok: false,
    kind: 'error:runtime.unavailable',
    error: 'The secure runtime is unavailable.',
  );
}

class NativeRuntimeWorker {
  NativeRuntimeWorker._(this._commandPort, this._events, this._isolate);

  static Future<NativeRuntimeWorker> start() async {
    final ready = ReceivePort();
    final isolate = await Isolate.spawn(_workerMain, <Object?>[
      ready.sendPort,
    ], debugName: 'torca-native-runtime-worker');
    final ports = await ready.first.timeout(const Duration(seconds: 15)) as Map;
    if (ports['error'] != null) {
      throw StateError(ports['error'] as String);
    }
    final commandPort = ports['commandPort'] as SendPort;
    final eventPort = ReceivePort();
    commandPort.send(<String, Object?>{'attachEvents': eventPort.sendPort});
    return NativeRuntimeWorker._(commandPort, eventPort, isolate);
  }

  final SendPort _commandPort;
  final ReceivePort _events;
  final Isolate _isolate;
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
    _commandPort.send(<String, Object?>{
      'invoke': request.encode(requestId),
      'reply': reply.sendPort,
    });
    final value = await reply.first;
    reply.close();
    return value as String;
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
      _commandPort.send(<String, Object?>{
        'shutdown': true,
        'reply': reply.sendPort,
      });
      await reply.first;
      reply.close();
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
    final queued = _requestTail.then((_) async {
      final reply = ReceivePort();
      _commandPort.send(<String, Object?>{
        'dispose': true,
        'reply': reply.sendPort,
      });
      await reply.first;
      reply.close();
    });
    _requestTail = queued.then<void>(
      (_) {},
      onError: (Object _, StackTrace __) {},
    );
    await queued;
    _events.close();
    _isolate.kill(priority: Isolate.immediate);
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
  var notificationCursor = 0;
  ready.send(<String, Object?>{'commandPort': commandPort.sendPort});
  commandPort.listen((message) {
    if (message is! Map) return;
    if (message['attachEvents'] is SendPort) {
      eventsPort = message['attachEvents'] as SendPort;
      void pollSnapshot() {
        final target = eventsPort;
        if (target == null) return;
        var next = const Duration(milliseconds: 250);
        try {
          final raw = bindings.invoke(
            handle,
            RuntimeRequestDto.snapshot.encode(
              'worker-poll-${DateTime.now().microsecondsSinceEpoch}',
            ),
            5000,
          );
          final decoded = jsonDecode(raw);
          if (decoded is Map && decoded['snapshot'] is Map) {
            final snapshot = decoded['snapshot'] as Map;
            target.send(jsonEncode(snapshot));
            if (snapshot['bootstrapPhase'] == 'ready' ||
                snapshot['bootstrapPhase'] == 'ready_for_profile') {
              next = const Duration(seconds: 1);
            }
          }
          final notificationRaw = bindings.invoke(
            handle,
            RuntimeRequestDto.notificationEvents(
              notificationCursor,
            ).encode('worker-events-${DateTime.now().microsecondsSinceEpoch}'),
            5000,
          );
          final notificationDecoded = jsonDecode(notificationRaw);
          final events = notificationDecoded is Map
              ? notificationDecoded['snapshot']
              : null;
          if (events is Map && events['events'] is List) {
            for (final event in events['events'] as List) {
              if (event is Map) {
                final cursor = event['cursor'];
                if (cursor is int && cursor > notificationCursor) {
                  notificationCursor = cursor;
                }
                target.send(jsonEncode(event));
              }
            }
            final afterCursor = events['afterCursor'];
            if (afterCursor is int && afterCursor > notificationCursor) {
              notificationCursor = afterCursor;
            }
          }
        } on Object {
          // The next scheduled poll retries after a transient native failure.
        }
        snapshotTimer = Timer(next, pollSnapshot);
      }

      snapshotTimer ??= Timer(Duration.zero, pollSnapshot);
      return;
    }
    if (message['dispose'] == true) {
      snapshotTimer?.cancel();
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
    try {
      final result = bindings.invoke(handle, raw, 10000);
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

AppSnapshotDto? _decodeSnapshot(String raw) {
  final value = jsonDecode(raw);
  if (value is! Map<String, dynamic>) return null;
  final identity = value['identity'];
  final transport = value['transport'] is Map<String, dynamic>
      ? value['transport'] as Map<String, dynamic>
      : const <String, dynamic>{};
  final navigationBadges = value['navigationBadges'] is Map<String, dynamic>
      ? value['navigationBadges'] as Map<String, dynamic>
      : const <String, dynamic>{};
  TransportIndicatorDto indicator(
    String name, {
    required String fallbackState,
  }) {
    final item = transport[name] is Map<String, dynamic>
        ? transport[name] as Map<String, dynamic>
        : const <String, dynamic>{};
    return TransportIndicatorDto(
      state: item['state'] as String? ?? fallbackState,
      code: item['code'] as String? ?? 'UNAVAILABLE',
      latencyMs: item['latencyMs'] as int?,
      lastActivityAtMs: item['lastActivityAtMs'] as int?,
      activitySequence: item['activitySequence'] as int? ?? 0,
    );
  }

  final bootstrapSteps =
      (value['bootstrapSteps'] is List<Object?>
              ? value['bootstrapSteps'] as List<Object?>
              : const <Object?>[])
          .whereType<Map<String, dynamic>>()
          .map(
            (item) => BootstrapStepDto(
              id: item['id'] as String? ?? '',
              state: item['state'] as String? ?? 'pending',
              code: item['code'] as String?,
              progress: item['progress'] as int? ?? 0,
              attempt: item['attempt'] as int? ?? 0,
              startedAtMs: item['startedAtMs'] as int?,
              lastProgressAtMs: item['lastProgressAtMs'] as int?,
              retryAtMs: item['retryAtMs'] as int?,
            ),
          )
          .toList(growable: false);
  final pairings =
      (value['pairings'] is List
              ? value['pairings'] as List<Object?>
              : const <Object?>[])
          .whereType<Map<String, dynamic>>()
          .map(
            (item) => PairingDto(
              id: item['id'] as String? ?? '',
              code: item['code'] as String? ?? '',
              role: item['role'] as String? ?? '',
              state: item['state'] as String? ?? '',
              expiresAtMs: item['expiresAtMs'] as int? ?? 0,
              localApproved: item['localApproved'] as bool? ?? false,
              remoteApproved: item['remoteApproved'] as bool? ?? false,
            ),
          )
          .toList(growable: false);
  final contacts =
      (value['contacts'] is List
              ? value['contacts'] as List<Object?>
              : const <Object?>[])
          .whereType<Map<String, dynamic>>()
          .map((item) {
            final health = item['peerHealth'] is Map<String, dynamic>
                ? item['peerHealth'] as Map<String, dynamic>
                : const <String, dynamic>{};
            return ContactDto(
              id: item['id'] as String? ?? '',
              displayName: item['displayName'] as String? ?? 'Contact',
              onionAddress: item['onionAddress'] as String? ?? '',
              status: item['status'] as String? ?? '',
              connectionState: item['connectionState'] as String? ?? '',
              safetyNumber: item['safetyNumber'] as String?,
              verificationStatus:
                  item['verificationStatus'] as String? ?? 'unverified',
              verifiedAtMs: item['verifiedAtMs'] as int?,
              peerHealth: PeerHealthDto(
                state: health['state'] as String? ?? 'disconnected',
                quality: health['quality'] as String? ?? 'unknown',
                rttMs: health['rttMs'] as int?,
                lastSuccessAtMs: health['lastSuccessAtMs'] as int?,
                consecutiveFailures: health['consecutiveFailures'] as int? ?? 0,
                reconnectAttempt: health['reconnectAttempt'] as int? ?? 0,
                lastActivityAtMs: health['lastActivityAtMs'] as int?,
                activitySequence: health['activitySequence'] as int? ?? 0,
              ),
            );
          })
          .toList(growable: false);
  final conversations =
      (value['conversations'] is List
              ? value['conversations'] as List
              : const <Object?>[])
          .whereType<Map<String, dynamic>>()
          .map(
            (item) => ConversationDto(
              id: item['id'] as String? ?? '',
              contactId: item['contactId'] as String? ?? '',
              status: item['status'] as String? ?? '',
              unreadCount: item['unreadCount'] as int? ?? 0,
              lastActivityAtMs: item['lastActivityAtMs'] as int? ?? 0,
              lastMessageBody: item['lastMessageBody'] as String?,
              lastMessageDirection: item['lastMessageDirection'] as String?,
              lastMessageStatus: item['lastMessageStatus'] as String?,
            ),
          )
          .toList(growable: false);
  final messages =
      (value['messages'] is List
              ? value['messages'] as List<Object?>
              : const <Object?>[])
          .whereType<Map<String, dynamic>>()
          .map(_decodeMessage)
          .toList(growable: false);
  final attachments =
      (value['attachments'] is List
              ? value['attachments'] as List<Object?>
              : const <Object?>[])
          .whereType<Map<String, dynamic>>()
          .map(
            (item) => AttachmentDto(
              id: item['id'] as String? ?? '',
              messageId: item['messageId'] as String? ?? '',
              name: item['name'] as String? ?? '',
              mediaType: item['mediaType'] as String? ?? '',
              size: item['size'] as int? ?? 0,
              status: item['status'] as String? ?? '',
              offset: item['offset'] as int? ?? 0,
            ),
          )
          .toList(growable: false);
  return AppSnapshotDto(
    runtimeId: value['runtimeId'] as String? ?? '',
    revision: value['revision'] as int? ?? 0,
    notificationCursor: value['notificationCursor'] as int? ?? 0,
    notificationsEnabled: value['notificationsEnabled'] as bool? ?? true,
    identity: identity is Map<String, dynamic>
        ? IdentityDto(
            displayName: identity['displayName'] as String?,
            fingerprint: identity['fingerprint'] as String?,
          )
        : null,
    torState: value['torState'] as String? ?? 'stopped',
    transport: TransportStatusDto(
      tor: indicator(
        'tor',
        fallbackState: value['torState'] as String? ?? 'stopped',
      ),
      relay: indicator('relay', fallbackState: 'unknown'),
    ),
    navigationBadges: NavigationBadgesDto(
      unreadMessages: navigationBadges['unreadMessages'] as int? ?? 0,
      newContacts: navigationBadges['newContacts'] as int? ?? 0,
      pairingAttention: navigationBadges['pairingAttention'] as int? ?? 0,
    ),
    onionAddress: value['onionAddress'] as String?,
    bootstrapPhase: value['bootstrapPhase'] as String? ?? 'starting',
    bootstrapSteps: bootstrapSteps,
    pairings: pairings,
    contacts: contacts,
    conversations: conversations,
    messages: messages,
    attachments: attachments,
  );
}

MessageDto _decodeMessage(Map<String, dynamic> value) => MessageDto(
  id: value['id'] as String? ?? '',
  conversationId: value['conversationId'] as String? ?? '',
  body: value['body'] as String? ?? '',
  direction: value['direction'] as String? ?? 'unknown',
  status: value['status'] as String? ?? 'unknown',
  replyToMessageId: value['replyToMessageId'] as String?,
  createdAtMs: value['createdAtMs'] as int? ?? 0,
  updatedAtMs: value['updatedAtMs'] as int? ?? 0,
  attemptCount: value['attemptCount'] as int? ?? 0,
);
