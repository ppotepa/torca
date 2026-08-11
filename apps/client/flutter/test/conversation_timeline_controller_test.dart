import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/screens/conversation_timeline_controller.dart';

import 'fake_engine_gateway.dart';

class _HistoryGateway extends FakeEngineGateway
    implements ConversationHistoryProvider {
  final Completer<ConversationPageDto> first = Completer<ConversationPageDto>();
  Completer<ConversationPageDto>? delayedSecond;
  int calls = 0;

  @override
  Future<ConversationPageDto> loadConversationPage(
    String conversationId, {
    MessageDto? before,
    int limit = 100,
  }) {
    calls++;
    if (calls == 1) return first.future;
    if (calls == 2 && delayedSecond != null) return delayedSecond!.future;
    return Future<ConversationPageDto>.value(
      const ConversationPageDto(
        messages: <MessageDto>[
          MessageDto(
            id: 'message',
            conversationId: 'conversation',
            body: 'arrived during refresh',
            direction: 'inbound',
            status: 'delivered',
            createdAtMs: 1,
            updatedAtMs: 1,
            attemptCount: 0,
          ),
        ],
        hasMore: false,
      ),
    );
  }

  @override
  Future<ConversationPageDto> searchConversation(
    String conversationId,
    String query, {
    int limit = 100,
  }) async =>
      const ConversationPageDto(messages: <MessageDto>[], hasMore: false);
}

void main() {
  test('refresh requested during a load is not dropped', () async {
    final gateway = _HistoryGateway();
    final timeline = ConversationTimelineController(
      gateway: gateway,
      conversationId: 'conversation',
    );

    final initializing = timeline.initialize();
    await timeline.refreshLatest();
    gateway.first.complete(
      const ConversationPageDto(messages: <MessageDto>[], hasMore: false),
    );
    await initializing;

    expect(gateway.calls, 2);
    expect(timeline.messages.single.body, 'arrived during refresh');
    timeline.dispose();
    await gateway.dispose();
  });

  test('background refresh never re-enters the empty-state spinner', () async {
    final gateway = _HistoryGateway();
    final timeline = ConversationTimelineController(
      gateway: gateway,
      conversationId: 'conversation',
    );
    final initializing = timeline.initialize();
    gateway.first.complete(
      const ConversationPageDto(messages: <MessageDto>[], hasMore: false),
    );
    await initializing;
    expect(timeline.loading, isFalse);

    final refresh = Completer<ConversationPageDto>();
    gateway.delayedSecond = refresh;
    final refreshing = timeline.refreshLatest();
    await Future<void>.delayed(Duration.zero);
    expect(timeline.loading, isFalse);

    refresh.complete(
      const ConversationPageDto(
        messages: <MessageDto>[
          MessageDto(
            id: 'message',
            conversationId: 'conversation',
            body: 'arrived during refresh',
            direction: 'inbound',
            status: 'delivered',
            createdAtMs: 1,
            updatedAtMs: 1,
            attemptCount: 0,
          ),
        ],
        hasMore: false,
      ),
    );
    await refreshing;
    expect(timeline.loading, isFalse);
    expect(timeline.messages.single.body, 'arrived during refresh');
    timeline.dispose();
    await gateway.dispose();
  });
}
