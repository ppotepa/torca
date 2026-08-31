import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/message_bubble.dart';

void main() {
  testWidgets('Android message bubbles align to opposite gutters', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(360, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: Column(
            children: <Widget>[
              MessageBubble(
                message: const MessageDto(
                  id: 'inbound',
                  conversationId: 'conversation',
                  body: 'Incoming message',
                  direction: 'inbound',
                  status: 'delivered',
                  createdAtMs: 1000,
                ),
                onLongPress: () {},
              ),
              MessageBubble(
                message: const MessageDto(
                  id: 'outbound',
                  conversationId: 'conversation',
                  body: 'Outgoing message',
                  direction: 'outbound',
                  status: 'sent',
                  createdAtMs: 2000,
                  sentAtMs: 2000,
                ),
                onLongPress: () {},
              ),
            ],
          ),
        ),
      ),
    );

    final inbound = tester.getRect(
      find.byKey(const ValueKey<String>('message-bubble-inbound')),
    );
    final outbound = tester.getRect(
      find.byKey(const ValueKey<String>('message-bubble-outbound')),
    );
    final outboundFooter = tester.getRect(
      find.byKey(const ValueKey<String>('message-footer-outbound')),
    );

    expect(inbound.left, closeTo(12, 0.1));
    expect(outbound.right, closeTo(348, 0.1));
    expect(inbound.width, lessThanOrEqualTo(282.3));
    expect(outbound.width, lessThanOrEqualTo(282.3));
    expect(outboundFooter.right, closeTo(outbound.right - 10, 0.1));
    expect(tester.takeException(), isNull);
  });

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

  testWidgets('message actions have an explicit accessible button', (
    tester,
  ) async {
    var opened = false;
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: MessageBubble(
            message: const MessageDto(
              id: 'actions',
              conversationId: '02',
              body: 'Hello',
              direction: 'inbound',
              status: 'delivered',
              createdAtMs: 1000,
            ),
            onLongPress: () => opened = true,
          ),
        ),
      ),
    );

    expect(find.byTooltip('Message actions'), findsOneWidget);
    await tester.tap(find.byTooltip('Message actions'));
    expect(opened, isTrue);
  });
}
