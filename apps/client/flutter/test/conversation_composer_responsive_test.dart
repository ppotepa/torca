import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/screens/conversation_screen.dart';
import 'package:torca_app/theme/app_theme.dart';

import 'fake_engine_gateway.dart';

void main() {
  testWidgets('conversation pane remains usable across the parity widths', (
    tester,
  ) async {
    const contact = ContactDto(
      id: 'responsive-contact',
      displayName: 'Alice',
      status: 'active',
      connectionState: 'ready',
    );
    const conversation = ConversationDto(
      id: 'responsive-conversation',
      contactId: 'responsive-contact',
      status: 'active',
    );
    final gateway = FakeEngineGateway(
      initialSnapshot: const AppSnapshotDto(
        identity: IdentityDto(id: 'local', displayName: 'Me'),
        contacts: <ContactDto>[contact],
        conversations: <ConversationDto>[conversation],
        bootstrapPhase: 'ready',
      ),
    );
    addTearDown(gateway.dispose);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    tester.view.devicePixelRatio = 1;

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: ConversationScreen(gateway: gateway, conversation: conversation),
      ),
    );

    for (final width in <double>[
      320,
      360,
      390,
      430,
      600,
      768,
      960,
      1200,
      1440,
    ]) {
      tester.view.physicalSize = Size(width, width < 600 ? 720 : 800);
      await tester.pumpAndSettle();
      expect(find.byType(TextField), findsOneWidget);
      expect(tester.takeException(), isNull, reason: 'width=$width');
    }
  });

  testWidgets('conversation composer keeps a usable field on a phone width', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(320, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    const contact = ContactDto(
      id: 'contact-1',
      displayName: 'Alice',
      status: 'active',
      connectionState: 'ready',
    );
    const conversation = ConversationDto(
      id: 'conversation-1',
      contactId: 'contact-1',
      status: 'active',
    );
    final gateway = FakeEngineGateway(
      initialSnapshot: const AppSnapshotDto(
        identity: IdentityDto(id: 'local', displayName: 'Me'),
        contacts: <ContactDto>[contact],
        conversations: <ConversationDto>[conversation],
        bootstrapPhase: 'ready',
      ),
    );
    addTearDown(gateway.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: ConversationScreen(gateway: gateway, conversation: conversation),
      ),
    );
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.byType(TextField), findsOneWidget);
    expect(tester.getSize(find.byType(TextField)).width, greaterThan(100));
  });

  testWidgets('identity changes keep the composer locked until verification', (
    tester,
  ) async {
    const contact = ContactDto(
      id: 'changed-contact',
      displayName: 'Alice',
      status: 'active',
      connectionState: 'ready',
      verificationStatus: 'identity_changed',
    );
    const conversation = ConversationDto(
      id: 'changed-conversation',
      contactId: 'changed-contact',
      status: 'active',
    );
    final gateway = FakeEngineGateway(
      initialSnapshot: const AppSnapshotDto(
        identity: IdentityDto(id: 'local', displayName: 'Me'),
        contacts: <ContactDto>[contact],
        conversations: <ConversationDto>[conversation],
        bootstrapPhase: 'ready',
      ),
    );
    addTearDown(gateway.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: ConversationScreen(gateway: gateway, conversation: conversation),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.text('Sending is paused until this contact is verified again.'),
      findsOneWidget,
    );
    final send = tester.widget<IconButton>(
      find.byWidgetPredicate(
        (widget) => widget is IconButton && widget.tooltip == 'Send message',
      ),
    );
    expect(send.onPressed, isNull);
  });
}
