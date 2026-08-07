import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/widgets/conversation_actions.dart';
import 'package:torca_app/widgets/message_actions.dart';

void main() {
  testWidgets('touch message actions use the shared action model', (tester) async {
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

  testWidgets('conversation actions reflect blocked state', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => FilledButton(
            onPressed: () => ConversationActionMenu.showTouch(
              context,
              blocked: true,
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
  });
}
