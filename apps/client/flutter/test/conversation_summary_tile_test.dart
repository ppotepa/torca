import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/widgets/conversation_summary_tile.dart';

void main() {
  testWidgets('conversation summary shows preview and unread badge', (tester) async {
    const conversation = ConversationDto(
      id: 'c1',
      contactId: 'p1',
      status: 'active',
      unreadCount: 3,
      lastActivityAtMs: 1700000000000,
      lastMessageBody: 'hello',
      lastMessageDirection: 'inbound',
      lastMessageStatus: 'delivered',
    );
    const contact = ContactDto(
      id: 'p1',
      displayName: 'Alice',
      onionAddress: 'example.onion',
      status: 'active',
      connectionState: 'ready',
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ConversationSummaryTile(
            conversation: conversation,
            contact: contact,
            selected: false,
            onTap: () {},
            onContactInfo: null,
            onLongPress: null,
            onSecondaryTapDown: null,
          ),
        ),
      ),
    );

    expect(find.text('Alice'), findsOneWidget);
    expect(find.text('hello'), findsOneWidget);
    expect(find.text('3'), findsOneWidget);
  });
}
