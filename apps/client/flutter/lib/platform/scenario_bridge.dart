import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:torca_avatar/torca_avatar.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import 'scenario_bridge_controller.dart';

/// Local-only control surface for automated Android soak runs.
///
/// This is deliberately enabled only in debug/profile builds and binds only
/// to loopback. The random token is written to the app cache so the harness
/// can retrieve it through `adb run-as` and then use `adb forward`.
class ScenarioBridge implements ScenarioBridgeController {
  ScenarioBridge(this._gateway);

  final EngineGateway _gateway;
  HttpServer? _server;
  String? _token;
  File? _discoveryFile;

  bool get isRunning => _server != null;

  Future<void> start() async {
    const soakMode = bool.fromEnvironment('TORCA_SOAK_MODE');
    if ((!kDebugMode && !soakMode) || !Platform.isAndroid || _server != null) {
      return;
    }
    final token = _randomToken();
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    await Directory.systemTemp.create(recursive: true);
    final discovery = File(
      '${Directory.systemTemp.path}${Platform.pathSeparator}torca-scenario.json',
    );
    await discovery.writeAsString(
      jsonEncode(<String, Object?>{
        'schema': 1,
        'port': server.port,
        'token': token,
      }),
    );
    _server = server;
    _token = token;
    _discoveryFile = discovery;
    server.listen(_handleRequest);
  }

  Future<void> dispose() async {
    final server = _server;
    _server = null;
    _token = null;
    await server?.close(force: true);
    final discovery = _discoveryFile;
    _discoveryFile = null;
    try {
      await discovery?.delete();
    } on Object {
      // Cleanup is best effort during process shutdown.
    }
  }

  Future<void> _handleRequest(HttpRequest request) async {
    if (request.method != 'POST' ||
        request.headers.value('x-torca-scenario-token') != _token) {
      request.response.statusCode = HttpStatus.unauthorized;
      await request.response.close();
      return;
    }
    try {
      final body = await utf8.decoder.bind(request).join();
      if (body.length > 64 * 1024)
        throw const FormatException('request too large');
      final value = jsonDecode(body);
      if (value is! Map<String, dynamic>)
        throw const FormatException('request must be an object');
      final result = await _execute(value);
      final payload = jsonEncode(<String, Object?>{
        'status': 'succeeded',
        'result': result,
      });
      request.response
        ..headers.contentType = ContentType.json
        ..statusCode = HttpStatus.ok
        // Explicit content length keeps the small loopback protocol from
        // falling back to HTTP chunked framing. The Rust harness consumes a
        // single JSON document and must not receive chunk trailers.
        ..contentLength = utf8.encode(payload).length
        ..write(payload);
    } on Object catch (error) {
      final payload = jsonEncode(<String, Object?>{
        'status': 'failed',
        'error': '$error',
      });
      request.response
        ..headers.contentType = ContentType.json
        ..statusCode = HttpStatus.badRequest
        ..contentLength = utf8.encode(payload).length
        ..write(payload);
    } finally {
      await request.response.close();
    }
  }

  Future<Object?> _execute(Map<String, dynamic> request) async {
    final operation = request['op'];
    if (operation == 'attachment.fixture') {
      final size = (request['size'] as num?)?.toInt() ?? 0;
      if (size < 1 || size > 5 * 1024 * 1024) {
        throw ArgumentError('fixture size must be between 1 byte and 5 MiB');
      }
      final file = File(
        '${Directory.systemTemp.path}${Platform.pathSeparator}torca-scenario-fixture.bin',
      );
      final sink = file.openWrite();
      final block = List<int>.filled(4096, 0x54);
      var remaining = size;
      while (remaining > 0) {
        final count = remaining < block.length ? remaining : block.length;
        sink.add(block.sublist(0, count));
        remaining -= count;
      }
      await sink.close();
      return <String, Object?>{'path': file.path, 'size': size};
    }
    if (operation == 'snapshot' || operation == 'diagnostics') {
      await _gateway.execute(const RefreshSnapshotCommandDto());
      return <String, Object?>{
        'snapshot': _snapshotJson(_gateway.snapshots.value),
      };
    }
    BridgeCommandDto command;
    switch (operation) {
      case 'pairing.create':
        command = const CreatePairingCommandDto();
      case 'pairing.join':
        command = JoinPairingCommandDto(
          code: _string(request, 'code'),
          ticket: request['ticket'] as String?,
        );
      case 'pairing.approve':
        command = ApprovePairingCommandDto(
          sessionIdHex: _string(request, 'sessionIdHex'),
        );
      case 'pairing.reject':
        command = RejectPairingCommandDto(
          sessionIdHex: _string(request, 'sessionIdHex'),
        );
      case 'pairing.cancel':
        command = CancelPairingCommandDto(
          sessionIdHex: _string(request, 'sessionIdHex'),
        );
      case 'profile.set':
        final identityId =
            _gateway.snapshots.value.identity?.id ?? 'local-device';
        final avatar = await AvatarRepository.instance.envelopeForDevice(
          identityId,
        );
        command = UpdateProfileCommandDto(
          displayName: _string(request, 'displayName'),
          avatarEnvelope:
              jsonDecode(jsonEncode(avatar.toJson())) as Map<String, Object?>,
        );
      case 'contact.rename':
        command = RenameContactCommandDto(
          contactIdHex: _string(request, 'contactIdHex'),
          displayName: _string(request, 'displayName'),
        );
      case 'message.send':
        command = QueueMessageCommandDto(
          conversationIdHex: _string(request, 'conversationIdHex'),
          body: _string(request, 'body'),
        );
      case 'attachment.queue':
        command = QueueAttachmentCommandDto(
          conversationIdHex: _string(request, 'conversationIdHex'),
          sourcePath: _string(request, 'sourcePath'),
          name: _string(request, 'name'),
          mediaType:
              request['mediaType'] as String? ?? 'application/octet-stream',
          size: (request['size'] as num?)?.toInt() ?? 0,
        );
      case 'radio.enable':
        command = SetRadioEnabledCommandDto(
          contactIdHex: _string(request, 'contactIdHex'),
          enabled: request['enabled'] as bool? ?? true,
        );
      case 'radio.begin':
        command = BeginRadioTransmissionCommandDto(
          contactIdHex: _string(request, 'contactIdHex'),
        );
      case 'radio.end':
        command = EndRadioTransmissionCommandDto(
          contactIdHex: _string(request, 'contactIdHex'),
        );
      default:
        throw ArgumentError('unsupported scenario operation: $operation');
    }
    final result = await _gateway.execute(command);
    if (!result.ok || result.kind.startsWith('error')) {
      throw StateError(result.errorCode ?? result.kind);
    }
    return <String, Object?>{
      'ok': result.ok,
      'kind': result.kind,
      'error': result.error,
      'snapshot': _snapshotJson(_gateway.snapshots.value),
    };
  }

  static String _string(Map<String, dynamic> request, String key) {
    final value = request[key];
    if (value is! String || value.isEmpty)
      throw ArgumentError('$key is required');
    return value;
  }

  static String _randomToken() {
    final random = Random.secure();
    return List<String>.generate(
      32,
      (_) => random.nextInt(16).toRadixString(16),
    ).join();
  }

  static Map<String, Object?> _snapshotJson(AppSnapshotDto snapshot) =>
      <String, Object?>{
        'runtimeId': snapshot.runtimeId,
        'revision': snapshot.revision,
        'bootstrapPhase': snapshot.bootstrapPhase,
        'torState': snapshot.torState,
        'transport': <String, Object?>{
          'tor': <String, Object?>{
            'state': snapshot.transport.tor.state,
            'code': snapshot.transport.tor.code,
          },
          'relay': <String, Object?>{
            'state': snapshot.transport.relay.state,
            'code': snapshot.transport.relay.code,
          },
          'peer': <String, Object?>{
            'state': snapshot.transport.peer.state,
            'code': snapshot.transport.peer.code,
          },
        },
        'identity': snapshot.identity == null
            ? null
            : <String, Object?>{
                'id': snapshot.identity!.id,
                'displayName': snapshot.identity!.displayName,
                'fingerprint': snapshot.identity!.fingerprint,
              },
        'pairings': snapshot.pairings
            .map(
              (pairing) => <String, Object?>{
                'id': pairing.id,
                'code': pairing.code,
                'inviteUri': pairing.inviteUri,
                'role': pairing.role,
                'state': pairing.state,
                'expiresAtMs': pairing.expiresAtMs,
                'localApproved': pairing.localApproved,
                'remoteApproved': pairing.remoteApproved,
              },
            )
            .toList(growable: false),
        'contacts': snapshot.contacts
            .map(
              (contact) => <String, Object?>{
                'id': contact.id,
                'displayName': contact.displayName,
                'status': contact.status,
                'connectionState': contact.connectionState,
                'presenceState': contact.presenceState,
                'peerHealth': <String, Object?>{
                  'state': contact.peerHealth.state,
                  'quality': contact.peerHealth.quality,
                  'activitySequence': contact.peerHealth.activitySequence,
                },
              },
            )
            .toList(growable: false),
        'conversations': snapshot.conversations
            .map(
              (conversation) => <String, Object?>{
                'id': conversation.id,
                'contactId': conversation.contactId,
                'status': conversation.status,
                'unreadCount': conversation.unreadCount,
                'lastMessageBody': conversation.lastMessageBody,
                'lastMessageDirection': conversation.lastMessageDirection,
                'lastMessageStatus': conversation.lastMessageStatus,
              },
            )
            .toList(growable: false),
        'messages': snapshot.messages
            .map(
              (message) => <String, Object?>{
                'id': message.id,
                'conversationId': message.conversationId,
                'body': message.body,
                'direction': message.direction,
                'status': message.status,
                'deliveredAtMs': message.deliveredAtMs,
              },
            )
            .toList(growable: false),
        'attachments': snapshot.attachments
            .map(
              (attachment) => <String, Object?>{
                'id': attachment.id,
                'messageId': attachment.messageId,
                'name': attachment.name,
                'size': attachment.size,
                'offset': attachment.offset,
                'status': attachment.status,
                'direction': attachment.direction,
              },
            )
            .toList(growable: false),
        'radio': <String, Object?>{
          'activeContactId': snapshot.radio.activeContactId,
          'session': snapshot.radio.session == null
              ? null
              : <String, Object?>{
                  'contactId': snapshot.radio.session!.contactId,
                  'state': snapshot.radio.session!.state,
                  'floor': snapshot.radio.session!.floor,
                  'burstElapsedMs': snapshot.radio.session!.burstElapsedMs,
                },
        },
      };
}
