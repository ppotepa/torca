import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/message_bubble.dart';

void main() {
  testWidgets('message bubble presents reply time and delivery state', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: MessageBubble(
            message: const MessageDto(
              id: '01',
              conversationId: '02',
              body: 'Hello',
              direction: 'outbound',
              status: 'delivered',
              replyToMessageId: '03',
              createdAtMs: 1000,
            ),
            quotedBody: 'Earlier message',
            onLongPress: () {},
          ),
        ),
      ),
    );

    expect(find.text('Hello'), findsOneWidget);
    expect(find.text('Earlier message'), findsOneWidget);
    expect(find.byIcon(Icons.done_all), findsOneWidget);
    expect(find.text('outbound'), findsNothing);
    expect(find.text('Delivered'), findsNothing);
  });
}
