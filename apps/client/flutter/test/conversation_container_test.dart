import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/screens/conversation_screen.dart';

void main() {
  testWidgets(
    'conversation keeps header and composer fixed while content scrolls',
    (tester) async {
      const headerKey = Key('conversation-header');
      const footerKey = Key('conversation-footer');
      const listKey = Key('conversation-content');

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              height: 360,
              child: ConversationContainer(
                header: const SizedBox(key: headerKey, height: 56),
                content: ListView.builder(
                  key: listKey,
                  itemExtent: 48,
                  itemCount: 30,
                  itemBuilder: (_, index) => Text('Message $index'),
                ),
                footer: const SizedBox(key: footerKey, height: 68),
              ),
            ),
          ),
        ),
      );

      final headerBefore = tester.getTopLeft(find.byKey(headerKey));
      final footerBefore = tester.getTopLeft(find.byKey(footerKey));
      final firstMessageBefore = tester.getTopLeft(find.text('Message 0'));

      await tester.drag(find.byKey(listKey), const Offset(0, -180));
      await tester.pumpAndSettle();

      expect(tester.getTopLeft(find.byKey(headerKey)), headerBefore);
      expect(tester.getTopLeft(find.byKey(footerKey)), footerBefore);
      expect(find.text('Message 0'), findsNothing);
      expect(find.text('Message 8'), findsOneWidget);
      expect(firstMessageBefore.dy, greaterThan(headerBefore.dy));
    },
  );
}
