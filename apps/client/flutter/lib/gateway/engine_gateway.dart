import 'package:flutter/foundation.dart';
import 'package:torca_avatar/torca_avatar.dart';

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
    this.resourceId = '',
  });

  factory RuntimeEventDto.fromJson(Map<String, dynamic> value) {
    final eventId = value['eventId'];
    final kind = value['kind'];
    final createdAt = value['createdAtMs'];
    if (eventId is! String || eventId.isEmpty) {
      throw const FormatException('Runtime event is missing eventId');
    }
    if (kind is! String || kind.isEmpty) {
      throw const FormatException('Runtime event is missing kind');
    }
    if (createdAt is! num) {
      throw const FormatException('Runtime event is missing createdAtMs');
    }
    return RuntimeEventDto(
      cursor: (value['cursor'] as num?)?.toInt() ?? 0,
      eventId: eventId,
      kind: kind,
      conversationId: value['conversationId'] as String? ?? '',
      contactDisplayName: value['contactDisplayName'] as String? ?? '',
      createdAtMs: createdAt.toInt(),
      title: value['title'] as String? ?? 'Torca',
      body: value['body'] as String? ?? '',
      resourceId: value['resourceId'] as String? ?? '',
    );
  }

  final int cursor;
  final String eventId;
  final String kind;
  final String conversationId;
  final String contactDisplayName;
  final int createdAtMs;
  final String title;
  final String body;
  final String resourceId;
}

abstract interface class EngineGateway {
  ValueListenable<AppSnapshotDto> get snapshots;
  Stream<RuntimeEventDto> get events;
  Future<void> sendLifecycle(String event);
  Future<BridgeResultDto> execute(BridgeCommandDto command);
  Future<String> diagnosticsJson();
  Future<String> diagnosticsLogTailsJson();
  Future<void> dispose();
}

/// Optional targeted avatar capability. The root snapshot intentionally carries
/// only descriptors; callers fetch the compressed genome only when rendering it.
abstract interface class AvatarGenomeProvider {
  Future<AvatarGenomeEnvelope?> loadAvatarGenome({String? identityId});
}

abstract interface class GatewayAvailability {
  bool get isAvailable;
  String? get failureReason;
}

class ClientBuildInfo {
  const ClientBuildInfo({
    required this.communicationProvider,
    required this.productVersion,
    required this.buildId,
    required this.sourceCommit,
    required this.sourceFingerprint,
    required this.providerEndpointHash,
    required this.providerEndpointRequired,
    this.providerProfile,
    required this.targetPlatform,
    required this.targetArchitecture,
    required this.contractSchema,
    required this.storageEpoch,
    required this.wireVersion,
    this.capabilities = const ClientCapabilitiesDto(),
  });

  factory ClientBuildInfo.fromJson(Map<String, dynamic> value) {
    String requiredString(String key) {
      final field = value[key];
      if (field is String && field.isNotEmpty) return field;
      throw FormatException('Runtime metadata is missing $key');
    }

    int requiredInt(String key) {
      final field = value[key];
      if (field is num) return field.toInt();
      throw FormatException('Runtime metadata is missing $key');
    }

    String? optionalString(String key) {
      final field = value[key];
      if (field == null) return null;
      if (field is String && field.isNotEmpty) return field;
      throw FormatException('Runtime metadata $key is invalid');
    }

    final metadataSchema = (value['metadataSchema'] as num?)?.toInt() ?? 1;
    final providerValue = value['communicationProvider'];
    if (metadataSchema >= 2 &&
        (!(providerValue is String) || providerValue.trim().isEmpty)) {
      throw const FormatException(
        'Runtime metadata is missing communicationProvider',
      );
    }

    final capabilities = value['capabilities'];
    if (capabilities != null && capabilities is! Map<String, dynamic>) {
      throw const FormatException('Runtime metadata capabilities are invalid');
    }
    final providerEndpointHash = optionalString('providerEndpointHash');
    final communicationProvider =
        (providerValue is String && providerValue.trim().isNotEmpty)
        ? providerValue.trim().toLowerCase()
        : 'iroh';
    if (communicationProvider != 'iroh') {
      throw FormatException(
        'Runtime metadata has unsupported communicationProvider '
        '$communicationProvider',
      );
    }
    final providerEndpointRequired = value['providerEndpointRequired'] is bool
        ? value['providerEndpointRequired'] as bool
        : false;
    if (providerEndpointRequired && providerEndpointHash == null) {
      throw const FormatException(
        'Runtime metadata is missing providerEndpointHash for managed rendezvous provider',
      );
    }
    return ClientBuildInfo(
      communicationProvider: communicationProvider,
      productVersion: requiredString('productVersion'),
      buildId: requiredString('buildId'),
      sourceCommit: requiredString('sourceCommit'),
      sourceFingerprint: requiredString('sourceFingerprint'),
      providerEndpointHash: providerEndpointHash,
      providerEndpointRequired: providerEndpointRequired,
      providerProfile: optionalString('providerProfile'),
      targetPlatform: requiredString('targetPlatform'),
      targetArchitecture: requiredString('targetArchitecture'),
      contractSchema: requiredInt('contractSchema'),
      storageEpoch: requiredInt('storageEpoch'),
      wireVersion: requiredInt('wireVersion'),
      capabilities: ClientCapabilitiesDto.fromJson(
        capabilities as Map<String, dynamic>? ?? const <String, dynamic>{},
      ),
    );
  }

  final String productVersion;
  final String communicationProvider;
  final String buildId;
  final String sourceCommit;
  final String sourceFingerprint;
  final String? providerEndpointHash;
  final bool providerEndpointRequired;
  final String? providerProfile;
  final String targetPlatform;
  final String targetArchitecture;
  final int contractSchema;
  final int storageEpoch;
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
    this.pairingQr = true,
    this.pairingFullLink = true,
    this.pairingShortCode = true,
    this.supportsIncoming = true,
    this.supportsRadio = true,
    this.supportsAttachments = true,
    this.providerDirectPath = false,
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
        pairingQr: value['pairingQr'] as bool? ?? true,
        pairingFullLink: value['pairingFullLink'] as bool? ?? true,
        pairingShortCode: value['pairingShortCode'] as bool? ?? true,
        supportsIncoming: value['supportsIncoming'] as bool? ?? true,
        supportsRadio: value['supportsRadio'] as bool? ?? true,
        supportsAttachments: value['supportsAttachments'] as bool? ?? true,
        providerDirectPath: value['providerDirectPath'] as bool? ?? false,
      );

  final int maxAttachmentBytes;
  final int maxVideoAttachmentBytes;
  final int maxQueuedAttachments;
  final int maxAttachmentSourceBytes;
  final bool pairingQr;
  final bool pairingFullLink;
  final bool pairingShortCode;
  final bool supportsIncoming;
  final bool supportsRadio;
  final bool supportsAttachments;
  final bool providerDirectPath;
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
    implements
        EngineGateway,
        RuntimeShutdownGateway,
        GatewayAvailability,
        AvatarGenomeProvider {
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
  Future<String> diagnosticsLogTailsJson() async => '{"logs":[]}';

  @override
  Future<AvatarGenomeEnvelope?> loadAvatarGenome({String? identityId}) async =>
      null;

  @override
  Future<void> shutdown() async {}

  @override
  Future<void> dispose() async {
    _snapshots.dispose();
  }
}
