import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/message_bubble.dart';

void main() {
  testWidgets('message bubble presents reply time and delivery state', (
    tester,
  ) async {
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
              status: 'read',
              replyToMessageId: '03',
              createdAtMs: 1000,
              sentAtMs: 2000,
              deliveredAtMs: 3000,
              readAtMs: 4000,
            ),
            senderLabel: 'You',
            quotedBody: 'Earlier message',
            onLongPress: () {},
          ),
        ),
      ),
    );

    expect(find.text('Hello'), findsOneWidget);
    expect(find.text('You'), findsOneWidget);
    expect(find.text('Earlier message'), findsOneWidget);
    // The compact footer presents exactly the furthest known receipt.  Earlier
    // milestones remain diagnostic detail, rather than duplicating Delivered
    // and Read in the message bubble.
    expect(
      find.byWidgetPredicate(
        (widget) =>
            widget is Tooltip && (widget.message?.startsWith('Read ') ?? false),
      ),
      findsOneWidget,
    );
    expect(
      find.byWidgetPredicate(
        (widget) =>
            widget is Tooltip &&
            ((widget.message?.startsWith('Sent ') ?? false) ||
                (widget.message?.startsWith('Delivered ') ?? false)),
      ),
      findsNothing,
    );
    expect(find.byTooltip('Read'), findsNothing);
    expect(find.text('outbound'), findsNothing);
    expect(find.text('Delivered'), findsNothing);
  });
}
