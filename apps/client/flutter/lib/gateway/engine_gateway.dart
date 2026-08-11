import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';

class RuntimeEventDto {
  const RuntimeEventDto({
    required this.cursor,
    required this.eventId,
    required this.kind,
    required this.conversationId,
    required this.contactDisplayName,
    required this.createdAtMs,
    required this.title,
    required this.body,
  });

  factory RuntimeEventDto.fromJson(Map<String, dynamic> value) =>
      RuntimeEventDto(
        cursor: (value['cursor'] as num?)?.toInt() ?? 0,
        eventId: value['eventId'] as String? ?? '',
        kind: value['kind'] as String? ?? '',
        conversationId: value['conversationId'] as String? ?? '',
        contactDisplayName: value['contactDisplayName'] as String? ?? '',
        createdAtMs: (value['createdAtMs'] as num?)?.toInt() ?? 0,
        title: value['title'] as String? ?? 'Torca',
        body: value['body'] as String? ?? '',
      );

  final int cursor;
  final String eventId;
  final String kind;
  final String conversationId;
  final String contactDisplayName;
  final int createdAtMs;
  final String title;
  final String body;
}

abstract interface class EngineGateway {
  ValueListenable<AppSnapshotDto> get snapshots;
  Stream<RuntimeEventDto> get events;
  Future<void> sendLifecycle(String event);
  Future<BridgeResultDto> execute(BridgeCommandDto command);
  Future<String> diagnosticsJson();
  Future<void> dispose();
}

abstract interface class GatewayAvailability {
  bool get isAvailable;
  String? get failureReason;
}

class ClientBuildInfo {
  const ClientBuildInfo({
    required this.productVersion,
    required this.buildId,
    required this.sourceCommit,
    required this.sourceFingerprint,
    required this.relayEndpointHash,
    required this.targetPlatform,
    required this.targetArchitecture,
    required this.contractSchema,
    required this.wireVersion,
    this.capabilities = const ClientCapabilitiesDto(),
  });

  factory ClientBuildInfo.fromJson(Map<String, dynamic> value) =>
      ClientBuildInfo(
        productVersion: value['productVersion'] as String? ?? 'development',
        buildId: value['buildId'] as String? ?? 'dev',
        sourceCommit: value['sourceCommit'] as String? ?? 'working-tree',
        sourceFingerprint:
            value['sourceFingerprint'] as String? ?? 'development',
        relayEndpointHash: value['relayEndpointHash'] as String? ?? 'unknown',
        targetPlatform: value['targetPlatform'] as String? ?? 'unknown',
        targetArchitecture: value['targetArchitecture'] as String? ?? 'unknown',
        contractSchema: value['contractSchema'] as int? ?? 0,
        wireVersion: value['wireVersion'] as int? ?? 0,
        capabilities: ClientCapabilitiesDto.fromJson(
          value['capabilities'] is Map<String, dynamic>
              ? value['capabilities'] as Map<String, dynamic>
              : const <String, dynamic>{},
        ),
      );

  final String productVersion;
  final String buildId;
  final String sourceCommit;
  final String sourceFingerprint;
  final String relayEndpointHash;
  final String targetPlatform;
  final String targetArchitecture;
  final int contractSchema;
  final int wireVersion;
  final ClientCapabilitiesDto capabilities;
}

abstract interface class BuildInfoProvider {
  ClientBuildInfo get buildInfo;
}

abstract interface class PairingUriParser {
  Future<String?> parsePairingUri(String rawUri);
}

abstract interface class PairingUriEncoder {
  Future<String?> encodePairingUri(String code);
}

class ClientCapabilitiesDto {
  const ClientCapabilitiesDto({
    this.maxAttachmentBytes = 16 * 1024 * 1024,
    this.maxVideoAttachmentBytes = 5 * 1024 * 1024,
    this.maxQueuedAttachments = 5,
    this.maxAttachmentSourceBytes = 64 * 1024 * 1024,
  });

  factory ClientCapabilitiesDto.fromJson(Map<String, dynamic> value) =>
      ClientCapabilitiesDto(
        maxAttachmentBytes:
            value['maxAttachmentBytes'] as int? ?? 16 * 1024 * 1024,
        maxVideoAttachmentBytes:
            value['maxVideoAttachmentBytes'] as int? ?? 5 * 1024 * 1024,
        maxQueuedAttachments: value['maxQueuedAttachments'] as int? ?? 5,
        maxAttachmentSourceBytes:
            value['maxAttachmentSourceBytes'] as int? ?? 64 * 1024 * 1024,
      );

  final int maxAttachmentBytes;
  final int maxVideoAttachmentBytes;
  final int maxQueuedAttachments;
  final int maxAttachmentSourceBytes;
}

abstract interface class AttachmentCapabilitiesProvider {
  ClientCapabilitiesDto get capabilities;
}

ClientCapabilitiesDto capabilitiesFor(EngineGateway gateway) =>
    gateway is AttachmentCapabilitiesProvider
    ? (gateway as AttachmentCapabilitiesProvider).capabilities
    : const ClientCapabilitiesDto();

class ConversationPageDto {
  const ConversationPageDto({required this.messages, required this.hasMore});

  final List<MessageDto> messages;
  final bool hasMore;
}

abstract interface class ConversationHistoryProvider {
  Future<ConversationPageDto> loadConversationPage(
    String conversationId, {
    MessageDto? before,
    int limit = 100,
  });

  Future<ConversationPageDto> searchConversation(
    String conversationId,
    String query, {
    int limit = 100,
  });
}

Future<ConversationPageDto> conversationPageFor(
  EngineGateway gateway,
  String conversationId, {
  MessageDto? before,
  int limit = 100,
}) async {
  if (gateway is ConversationHistoryProvider) {
    return (gateway as ConversationHistoryProvider).loadConversationPage(
      conversationId,
      before: before,
      limit: limit,
    );
  }
  // History is a paginated Rust query. No UI-side filtering of root snapshots
  // is allowed, even for gateways that do not expose the optional capability.
  return const ConversationPageDto(messages: <MessageDto>[], hasMore: false);
}

Future<ConversationPageDto> searchConversationFor(
  EngineGateway gateway,
  String conversationId,
  String query, {
  int limit = 100,
}) async {
  if (gateway is ConversationHistoryProvider) {
    return (gateway as ConversationHistoryProvider).searchConversation(
      conversationId,
      query,
      limit: limit,
    );
  }
  return const ConversationPageDto(messages: <MessageDto>[], hasMore: false);
}

/// Optional host capability used only for an explicit application-level Quit.
abstract interface class RuntimeShutdownGateway {
  Future<void> shutdown();
}

class StartupFailureGateway
    implements EngineGateway, RuntimeShutdownGateway, GatewayAvailability {
  StartupFailureGateway(this.reason);

  final String reason;
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto());

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Stream<RuntimeEventDto> get events => const Stream<RuntimeEventDto>.empty();

  @override
  bool get isAvailable => false;

  @override
  String get failureReason => reason;

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    return BridgeResultDto(ok: false, kind: 'error', error: reason);
  }

  @override
  Future<void> sendLifecycle(String event) async {}

  @override
  Future<String> diagnosticsJson() async => '{"events":[]}';

  @override
  Future<void> shutdown() async {}

  @override
  Future<void> dispose() async {
    _snapshots.dispose();
  }
}
