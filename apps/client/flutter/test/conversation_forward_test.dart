import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/screens/conversation_screen.dart';
import 'package:torca_app/theme/app_theme.dart';

import 'fake_engine_gateway.dart';

class _HistoryGateway extends FakeEngineGateway
    implements ConversationHistoryProvider {
  _HistoryGateway()
    : super(
        initialSnapshot: const AppSnapshotDto(
          identity: IdentityDto(id: 'local', displayName: 'Me'),
          contacts: <ContactDto>[
            ContactDto(
              id: 'alice',
              displayName: 'Alice',
              status: 'active',
              connectionState: 'ready',
            ),
            ContactDto(
              id: 'bob',
              displayName: 'Bob',
              status: 'active',
              connectionState: 'ready',
            ),
          ],
          conversations: <ConversationDto>[
            ConversationDto(id: 'source', contactId: 'alice', status: 'active'),
            ConversationDto(id: 'target', contactId: 'bob', status: 'active'),
          ],
          bootstrapPhase: 'ready',
        ),
      );

  static const MessageDto sourceMessage = MessageDto(
    id: 'message-1',
    conversationId: 'source',
    body: 'Hello from Alice',
    direction: 'inbound',
    status: 'delivered',
    createdAtMs: 1,
  );

  @override
  Future<ConversationPageDto> loadConversationPage(
    String conversationId, {
    MessageDto? before,
    int limit = 100,
  }) async => conversationId == 'source'
      ? const ConversationPageDto(
          messages: <MessageDto>[sourceMessage],
          hasMore: false,
        )
      : const ConversationPageDto(messages: <MessageDto>[], hasMore: false);

  @override
  Future<ConversationPageDto> searchConversation(
    String conversationId,
    String query, {
    int limit = 100,
  }) async =>
      const ConversationPageDto(messages: <MessageDto>[], hasMore: false);
}

class _LifecycleHistoryGateway extends FakeEngineGateway
    implements ConversationHistoryProvider {
  _LifecycleHistoryGateway() : super(initialSnapshot: _snapshot(_sentMessage));

  static const MessageDto _sentMessage = MessageDto(
    id: 'message-status',
    conversationId: 'source',
    body: 'Lifecycle test',
    direction: 'outbound',
    status: 'sent',
    createdAtMs: 1000,
    updatedAtMs: 2000,
    sentAtMs: 2000,
  );

  MessageDto currentMessage = _sentMessage;

  static AppSnapshotDto _snapshot(MessageDto message) => AppSnapshotDto(
    identity: const IdentityDto(id: 'local', displayName: 'Me'),
    contacts: const <ContactDto>[
      ContactDto(
        id: 'alice',
        displayName: 'Alice',
        status: 'active',
        connectionState: 'ready',
      ),
    ],
    conversations: const <ConversationDto>[
      ConversationDto(
        id: 'source',
        contactId: 'alice',
        status: 'active',
        lastActivityAtMs: 1000,
      ),
    ],
    messages: <MessageDto>[message],
    bootstrapPhase: 'ready',
  );

  void publishRead() {
    currentMessage = const MessageDto(
      id: 'message-status',
      conversationId: 'source',
      body: 'Lifecycle test',
      direction: 'outbound',
      status: 'read',
      createdAtMs: 1000,
      updatedAtMs: 4000,
      sentAtMs: 2000,
      deliveredAtMs: 3000,
      readAtMs: 4000,
    );
    publish(_snapshot(currentMessage));
  }

  @override
  Future<ConversationPageDto> loadConversationPage(
    String conversationId, {
    MessageDto? before,
    int limit = 100,
  }) async => ConversationPageDto(
    messages: <MessageDto>[currentMessage],
    hasMore: false,
  );

  @override
  Future<ConversationPageDto> searchConversation(
    String conversationId,
    String query, {
    int limit = 100,
  }) async =>
      const ConversationPageDto(messages: <MessageDto>[], hasMore: false);
}

void main() {
  testWidgets('receipt-only snapshot refreshes the visible message footer', (
    tester,
  ) async {
    final gateway = _LifecycleHistoryGateway();
    addTearDown(gateway.dispose);
    const conversation = ConversationDto(
      id: 'source',
      contactId: 'alice',
      status: 'active',
      lastActivityAtMs: 1000,
    );

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: ConversationScreen(gateway: gateway, conversation: conversation),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.textContaining('Sent'), findsOneWidget);

    gateway.publishRead();
    await tester.pumpAndSettle();

    expect(find.textContaining('Seen at'), findsOneWidget);
  });

  testWidgets('forwarding queues the message from the explicit action button', (
    tester,
  ) async {
    final gateway = _HistoryGateway();
    addTearDown(gateway.dispose);
    const conversation = ConversationDto(
      id: 'source',
      contactId: 'alice',
      status: 'active',
    );

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: ConversationScreen(gateway: gateway, conversation: conversation),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(
      find.byKey(const ValueKey<String>('message-actions-message-1')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Forward message'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Bob'));
    await tester.pumpAndSettle();
    final queued = gateway.commands.whereType<QueueMessageCommandDto>();
    expect(queued, hasLength(1));
    expect(queued.single.conversationIdHex, 'target');
    expect(queued.single.body, 'Hello from Alice');
  });
}
