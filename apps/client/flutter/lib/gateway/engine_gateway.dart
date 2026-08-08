import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';

abstract interface class EngineGateway {
  ValueListenable<AppSnapshotDto> get snapshots;
  Future<BridgeResultDto> execute(BridgeCommandDto command);
  Future<String> diagnosticsJson();
  Future<void> dispose();
}

class AppCapabilities {
  const AppCapabilities({required this.maxAttachmentBytes});
  final int maxAttachmentBytes;
}

abstract interface class AttachmentCapabilitiesProvider {
  AppCapabilities get capabilities;
}

AppCapabilities capabilitiesFor(EngineGateway gateway) =>
    gateway is AttachmentCapabilitiesProvider
        ? gateway.capabilities
        : const AppCapabilities(maxAttachmentBytes: 16 * 1024 * 1024);

class ConversationPageDto {
  const ConversationPageDto({
    required this.messages,
    required this.hasMore,
  });

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
    return gateway.loadConversationPage(
      conversationId,
      before: before,
      limit: limit,
    );
  }
  final all = gateway.snapshots.value.messages
      .where((message) => message.conversationId == conversationId)
      .toList(growable: false)
    ..sort(_messageOrder);
  final filtered = before == null
      ? all
      : all
          .where((message) => _messageOrder(message, before) < 0)
          .toList(growable: false);
  final bounded = limit.clamp(1, 200);
  final start = filtered.length > bounded ? filtered.length - bounded : 0;
  return ConversationPageDto(
    messages: filtered.sublist(start),
    hasMore: start > 0,
  );
}

Future<ConversationPageDto> searchConversationFor(
  EngineGateway gateway,
  String conversationId,
  String query, {
  int limit = 100,
}) async {
  if (gateway is ConversationHistoryProvider) {
    return gateway.searchConversation(conversationId, query, limit: limit);
  }
  final normalized = query.trim().toLowerCase();
  if (normalized.isEmpty) {
    return const ConversationPageDto(messages: <MessageDto>[], hasMore: false);
  }
  final messages = gateway.snapshots.value.messages
      .where(
        (message) =>
            message.conversationId == conversationId &&
            message.body.toLowerCase().contains(normalized),
      )
      .toList(growable: false)
    ..sort(_messageOrder);
  final bounded = limit.clamp(1, 200);
  return ConversationPageDto(
    messages: messages.length <= bounded
        ? messages
        : messages.sublist(messages.length - bounded),
    hasMore: messages.length > bounded,
  );
}

int _messageOrder(MessageDto first, MessageDto second) {
  final byTime = first.createdAtMs.compareTo(second.createdAtMs);
  return byTime != 0 ? byTime : first.id.compareTo(second.id);
}

/// Optional host capability used only for an explicit application-level Quit.
abstract interface class ProcessRuntimeControl {
  Future<void> shutdown();
}

class UnavailableEngineGateway implements EngineGateway, ProcessRuntimeControl {
  UnavailableEngineGateway(this.reason);

  final String reason;
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto());

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    return BridgeResultDto(ok: false, kind: 'error', error: reason);
  }

  @override
  Future<String> diagnosticsJson() async => '{"events":[]}';

  @override
  Future<void> shutdown() async {}

  @override
  Future<void> dispose() async {
    _snapshots.dispose();
  }
}
