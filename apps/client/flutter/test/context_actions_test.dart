import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/widgets/conversation_actions.dart';
import 'package:torca_app/widgets/message_actions.dart';

void main() {
  testWidgets('touch message actions use the shared action model', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => FilledButton(
            onPressed: () => MessageActionMenu.showTouch(context),
            child: const Text('Open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(find.text('Reply'), findsOneWidget);
    expect(find.text('Copy'), findsOneWidget);
    expect(find.text('Message details'), findsOneWidget);
  });

  testWidgets('cancel is offered only for cancellable message jobs', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => FilledButton(
            onPressed: () => MessageActionMenu.showTouch(
              context,
              canCancel: true,
              canEdit: true,
            ),
            child: const Text('Open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(find.text('Cancel message'), findsOneWidget);
    expect(find.text('Edit message'), findsOneWidget);
    expect(find.text('Forward message'), findsOneWidget);
  });

  testWidgets('conversation actions reflect blocked state', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => FilledButton(
            onPressed: () => ConversationActionMenu.showTouch(
              context,
              blocked: true,
              archived: false,
              pinned: false,
              muted: false,
              unread: true,
            ),
            child: const Text('Actions'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Actions'));
    await tester.pumpAndSettle();
    expect(find.text('Unblock contact'), findsOneWidget);
    expect(find.text('Remove contact'), findsOneWidget);
    expect(find.text('Clear conversation history'), findsOneWidget);
    expect(find.text('Archive conversation'), findsOneWidget);
    expect(find.text('Pin conversation'), findsOneWidget);
    expect(find.text('Mute conversation'), findsOneWidget);
    expect(find.text('Mark as read'), findsOneWidget);
  });

  testWidgets('conversation actions expose mark as read only when unread', (
    tester,
  ) async {
    ConversationAction? selected;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => FilledButton(
            onPressed: () async {
              selected = await ConversationActionMenu.showTouch(
                context,
                blocked: false,
                archived: false,
                pinned: false,
                muted: false,
                unread: true,
              );
            },
            child: const Text('Open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Mark as read'));
    await tester.pumpAndSettle();
    expect(selected, ConversationAction.markRead);
  });
}
