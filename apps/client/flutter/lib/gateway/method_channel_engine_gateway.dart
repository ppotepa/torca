import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../generated/torca_contract.dart';
import 'engine_gateway.dart';

/// Production Flutter transport for the native Torca engine host.
///
/// Windows and Android hosts must implement the `torca.engine.v1` method channel.
/// The in-memory gateway is not selected unless a developer explicitly enables it.
class MethodChannelEngineGateway implements EngineGateway {
  MethodChannelEngineGateway({MethodChannel? channel})
      : _channel = channel ?? const MethodChannel(channelName) {
    _channel.setMethodCallHandler(_handleNativeCall);
  }

  static const String channelName = 'torca.engine.v1';

  final MethodChannel _channel;
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto());
  bool _disposed = false;

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  /// Loads the first engine snapshot. Host absence is returned as a typed error.
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

    try {
      final Object? rawResult = await _channel.invokeMethod<Object?>(
        'execute',
        <String, Object?>{
          'contractVersion': torcaContractVersion,
          'command': _encodeCommand(command),
        },
      );
      final BridgeResultDto result = _decodeResult(rawResult);
      if (result.ok) {
        await _refreshSnapshot();
      }
      return result;
    } on MissingPluginException {
      return _hostUnavailable();
    } on PlatformException catch (error) {
      return BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'native engine error: ${error.code}',
      );
    } on FormatException catch (error) {
      return BridgeResultDto(
        ok: false,
        kind: 'error',
        error: error.message,
      );
    }
  }

  Future<BridgeResultDto> _refreshSnapshot() async {
    if (_disposed) {
      return const BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'native engine gateway is disposed',
      );
    }

    try {
      final Object? rawSnapshot = await _channel.invokeMethod<Object?>(
        'snapshot',
        <String, Object?>{'contractVersion': torcaContractVersion},
      );
      _snapshots.value = _decodeSnapshot(rawSnapshot);
      return const BridgeResultDto(ok: true, kind: 'snapshot');
    } on MissingPluginException {
      return _hostUnavailable();
    } on PlatformException catch (error) {
      return BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'native engine error: ${error.code}',
      );
    } on FormatException catch (error) {
      return BridgeResultDto(
        ok: false,
        kind: 'error',
        error: error.message,
      );
    }
  }

  Future<Object?> _handleNativeCall(MethodCall call) async {
    if (_disposed) {
      return null;
    }
    switch (call.method) {
      case 'snapshotChanged':
        _snapshots.value = _decodeSnapshot(call.arguments);
        return <String, Object?>{'accepted': true};
      default:
        throw MissingPluginException(
          'Unsupported native engine callback: ${call.method}',
        );
    }
  }

  Map<String, Object?> _encodeCommand(BridgeCommandDto command) {
    if (command is CreateIdentityCommandDto) {
      return <String, Object?>{
        'type': 'createIdentity',
        'identityIdHex': command.identityIdHex,
        'displayName': command.displayName,
        'atMs': command.atMs,
      };
    }
    if (command is StartPairingCommandDto) {
      return <String, Object?>{
        'type': 'startPairing',
        'sessionIdHex': command.sessionIdHex,
        'code': command.code,
        'expiresAtMs': command.expiresAtMs,
      };
    }
    if (command is QueueMessageCommandDto) {
      return <String, Object?>{
        'type': 'queueMessage',
        'messageIdHex': command.messageIdHex,
        'conversationIdHex': command.conversationIdHex,
        'body': command.body,
        'atMs': command.atMs,
      };
    }
    throw const FormatException('unsupported bridge command');
  }

  BridgeResultDto _decodeResult(Object? value) {
    final Map<Object?, Object?> map = _requireMap(value, 'bridge result');
    return BridgeResultDto(
      ok: _requireBool(map, 'ok'),
      kind: _requireString(map, 'kind'),
      error: _optionalString(map, 'error'),
    );
  }

  AppSnapshotDto _decodeSnapshot(Object? value) {
    final Map<Object?, Object?> map = _requireMap(value, 'app snapshot');
    final int version = _requireInt(map, 'contractVersion');
    if (version != torcaContractVersion) {
      throw FormatException(
        'unsupported native contract version $version',
      );
    }

    final Object? identityValue = map['identity'];
    final IdentityDto? identity = identityValue == null
        ? null
        : IdentityDto(
            displayName: _requireString(
              _requireMap(identityValue, 'identity'),
              'displayName',
            ),
          );

    return AppSnapshotDto(
      identity: identity,
      contacts: _requireList(map, 'contacts')
          .map<ContactDto>(_decodeContact)
          .toList(growable: false),
      conversations: _requireList(map, 'conversations')
          .map<ConversationDto>(_decodeConversation)
          .toList(growable: false),
      messages: _requireList(map, 'messages')
          .map<MessageDto>(_decodeMessage)
          .toList(growable: false),
    );
  }

  ContactDto _decodeContact(Object? value) {
    final Map<Object?, Object?> map = _requireMap(value, 'contact');
    return ContactDto(
      id: _requireString(map, 'id'),
      onionAddress: _requireString(map, 'onionAddress'),
      status: _requireString(map, 'status'),
    );
  }

  ConversationDto _decodeConversation(Object? value) {
    final Map<Object?, Object?> map = _requireMap(value, 'conversation');
    return ConversationDto(
      id: _requireString(map, 'id'),
      contactId: _requireString(map, 'contactId'),
      status: _requireString(map, 'status'),
    );
  }

  MessageDto _decodeMessage(Object? value) {
    final Map<Object?, Object?> map = _requireMap(value, 'message');
    return MessageDto(
      id: _requireString(map, 'id'),
      conversationId: _requireString(map, 'conversationId'),
      body: _requireString(map, 'body'),
      direction: _requireString(map, 'direction'),
      status: _requireString(map, 'status'),
    );
  }

  Map<Object?, Object?> _requireMap(Object? value, String field) {
    if (value is Map<Object?, Object?>) {
      return value;
    }
    throw FormatException('$field must be a map');
  }

  List<Object?> _requireList(Map<Object?, Object?> map, String field) {
    final Object? value = map[field];
    if (value is List<Object?>) {
      return value;
    }
    throw FormatException('$field must be a list');
  }

  String _requireString(Map<Object?, Object?> map, String field) {
    final Object? value = map[field];
    if (value is String) {
      return value;
    }
    throw FormatException('$field must be a string');
  }

  String? _optionalString(Map<Object?, Object?> map, String field) {
    final Object? value = map[field];
    if (value == null || value is String) {
      return value as String?;
    }
    throw FormatException('$field must be a string or null');
  }

  bool _requireBool(Map<Object?, Object?> map, String field) {
    final Object? value = map[field];
    if (value is bool) {
      return value;
    }
    throw FormatException('$field must be a bool');
  }

  int _requireInt(Map<Object?, Object?> map, String field) {
    final Object? value = map[field];
    if (value is int) {
      return value;
    }
    throw FormatException('$field must be an int');
  }

  BridgeResultDto _hostUnavailable() {
    return const BridgeResultDto(
      ok: false,
      kind: 'error',
      error: 'native Torca engine host is unavailable',
    );
  }

  @override
  Future<void> dispose() async {
    if (_disposed) {
      return;
    }
    _disposed = true;
    await _channel.setMethodCallHandler(null);
    _snapshots.dispose();
  }
}
