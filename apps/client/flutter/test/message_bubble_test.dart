import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/message_bubble.dart';
import 'package:torca_ui/torca_ui.dart';

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
    // The footer stays inside the bubble border in both modern and terminal
    // appearances.
    expect(outboundFooter.right, closeTo(outbound.right - 10 - 1.2, 0.1));
    expect(tester.takeException(), isNull);
  });

  testWidgets('message body and footer stay visible across parity widths', (
    tester,
  ) async {
    const widths = <double>[320, 360, 390, 430, 600, 768, 960, 1200, 1440];
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    tester.view.devicePixelRatio = 1;

    for (final width in widths) {
      for (final appearance in <TorcaAppearance>[
        const TorcaAppearance(),
        const TorcaAppearance(
          family: TorcaThemeFamily.terminal,
          variant: TorcaThemeVariant.terminalTokyoNight,
        ),
      ]) {
        tester.view.physicalSize = Size(width, 800);
        await tester.pumpWidget(
          MaterialApp(
            theme: AppTheme.dark(appearance),
            home: Scaffold(
              body: MessageBubble(
                message: const MessageDto(
                  id: 'responsive',
                  conversationId: 'conversation',
                  body:
                      'A long message with a URL https://example.test/path and emoji 🎙️',
                  direction: 'inbound',
                  status: 'delivered',
                  createdAtMs: 1000,
                ),
                senderLabel: 'Contact',
                onLongPress: () {},
              ),
            ),
          ),
        );

        final bubble = tester.getRect(
          find.byKey(const ValueKey<String>('message-bubble-responsive')),
        );
        expect(bubble.right, lessThanOrEqualTo(width + .1));
        expect(
          find.byKey(const ValueKey<String>('message-content-responsive')),
          findsOneWidget,
        );
        expect(
          find.byKey(const ValueKey<String>('message-footer-responsive')),
          findsOneWidget,
        );
        expect(tester.takeException(), isNull);
      }
    }
  });

  for (final appearance in <TorcaAppearance>[
    const TorcaAppearance(),
    const TorcaAppearance(
      family: TorcaThemeFamily.terminal,
      variant: TorcaThemeVariant.terminalTokyoNight,
    ),
  ]) {
    testWidgets(
      'message has visible body and footer in ${appearance.family.name}',
      (tester) async {
        await tester.pumpWidget(
          MaterialApp(
            theme: AppTheme.dark(appearance),
            home: Scaffold(
              body: MessageBubble(
                message: const MessageDto(
                  id: 'body-footer',
                  conversationId: 'conversation',
                  body: 'A message body',
                  direction: 'outbound',
                  status: 'delivered',
                  createdAtMs: 1000,
                ),
                onLongPress: () {},
              ),
            ),
          ),
        );

        expect(
          find.byKey(const ValueKey<String>('message-body-body-footer')),
          findsOneWidget,
        );
        expect(
          find.byKey(const ValueKey<String>('message-footer-body-footer')),
          findsOneWidget,
        );
        expect(find.text('A message body'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );
  }

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

  testWidgets('grouped message keeps sender, body and footer', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: MessageBubble(
            message: const MessageDto(
              id: 'grouped',
              conversationId: '02',
              body: 'Second message',
              direction: 'outbound',
              status: 'sent',
              createdAtMs: 2000,
              sentAtMs: 2000,
            ),
            senderLabel: null,
            compactTop: true,
            onLongPress: () {},
          ),
        ),
      ),
    );

    expect(find.text('Second message'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('message-footer-grouped')),
      findsOneWidget,
    );
    expect(find.text('You'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('grouped messages show the sender only at the block start', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: Column(
            children: <Widget>[
              MessageBubble(
                message: const MessageDto(
                  id: 'block-first',
                  conversationId: '02',
                  body: 'First',
                  direction: 'inbound',
                  status: 'delivered',
                  createdAtMs: 1000,
                ),
                senderLabel: 'Alex',
                onLongPress: () {},
              ),
              MessageBubble(
                message: const MessageDto(
                  id: 'block-second',
                  conversationId: '02',
                  body: 'Second',
                  direction: 'inbound',
                  status: 'delivered',
                  createdAtMs: 2000,
                ),
                senderLabel: 'Alex',
                compactTop: true,
                showSender: false,
                onLongPress: () {},
              ),
            ],
          ),
        ),
      ),
    );

    expect(find.text('Alex'), findsOneWidget);
    expect(find.text('First'), findsOneWidget);
    expect(find.text('Second'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('message-header-block-second')),
      findsOneWidget,
    );
  });

  testWidgets('footer aggregates active reactions and hides inactive ones', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: MessageBubble(
            message: const MessageDto(
              id: 'reactions',
              conversationId: '02',
              body: 'React to this',
              direction: 'inbound',
              status: 'delivered',
              createdAtMs: 1725106800000,
            ),
            reactions: const <ReactionDto>[
              ReactionDto(
                messageId: 'reactions',
                conversationId: '02',
                actorId: 'one',
                emoji: '❤️',
                active: true,
              ),
              ReactionDto(
                messageId: 'reactions',
                conversationId: '02',
                actorId: 'two',
                emoji: '❤️',
                active: true,
              ),
              ReactionDto(
                messageId: 'reactions',
                conversationId: '02',
                actorId: 'three',
                emoji: '👍',
                active: false,
              ),
            ],
            onLongPress: () {},
          ),
        ),
      ),
    );

    expect(find.text('❤️ 2'), findsOneWidget);
    expect(find.text('👍'), findsNothing);
  });

  testWidgets('grouped deleted messages keep body and footer sections', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.dark(
          const TorcaAppearance(family: TorcaThemeFamily.terminal),
        ),
        home: Scaffold(
          body: MessageBubble(
            message: const MessageDto(
              id: 'deleted-grouped',
              conversationId: '02',
              body: 'Secret text',
              direction: 'inbound',
              status: 'deleted',
              createdAtMs: 1000,
            ),
            showBody: false,
            compactTop: true,
            compactBottom: true,
            showSender: false,
            senderLabel: 'Alex',
            onLongPress: () {},
          ),
        ),
      ),
    );

    expect(find.text('Secret text'), findsNothing);
    expect(find.text('Message deleted'), findsOneWidget);
    expect(
      find.byKey(
        const ValueKey<String>('message-body-section-deleted-grouped'),
      ),
      findsOneWidget,
    );
    expect(
      find.byKey(
        const ValueKey<String>('message-footer-section-deleted-grouped'),
      ),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });
}
