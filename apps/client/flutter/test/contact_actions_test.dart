import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/widgets/contact_actions.dart';

import 'fake_engine_gateway.dart';

void main() {
  testWidgets('rename keeps the dialog field alive through route dismissal', (
    WidgetTester tester,
  ) async {
    const contact = ContactDto(
      id: 'contact-1',
      displayName: 'Alice',
      status: 'active',
      connectionState: 'ready',
    );
    final gateway = FakeEngineGateway();
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Scaffold(
            body: FilledButton(
              onPressed: () => ContactActions.rename(context, gateway, contact),
              child: const Text('Rename'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Rename'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextFormField), 'Bob');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    final command = gateway.commands
        .whereType<RenameContactCommandDto>()
        .single;
    expect(command.displayName, 'Bob');
  });
}
