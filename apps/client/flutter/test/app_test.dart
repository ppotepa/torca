import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/localization/app_locale_mode.dart';
import 'package:torca_app/navigation/app_navigation_controller.dart';
import 'package:torca_app/screens/conversation_screen.dart';
import 'package:torca_app/screens/pairing_screen.dart';
import 'package:torca_app/settings/local_preferences.dart';
import 'package:torca_app/widgets/pairing_modal_registry.dart';
import 'package:torca_avatar/torca_avatar.dart';
import 'package:torca_ui/torca_ui.dart';
import 'fake_engine_gateway.dart';

TorcaApp _app(EngineGateway gateway, {LocalPreferences? preferences}) =>
    TorcaApp(
      gateway: gateway,
      navigation: AppNavigationController(),
      preferences: preferences ?? LocalPreferences(),
    );

void main() {
  testWidgets('generator modal starts work without a second generate action', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Scaffold(
            body: FilledButton(
              onPressed: () =>
                  showInvitationGeneratorModal(context, FakeEngineGateway()),
              child: const Text('Open generator'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open generator'));
    await tester.pump();

    expect(find.text('Your invitation'), findsOneWidget);
    expect(find.text('Generate Invitation'), findsNothing);
  });

  testWidgets('generator renders QR from the create response before snapshot', (
    WidgetTester tester,
  ) async {
    const id = '00000000000000000000000000000082';
    final gateway = FakeEngineGateway(
      responses: <FakeGatewayResponse>[
        FakeGatewayResponse.success(
          kind: 'pairing_started',
          resourceId: id,
          inviteUri: 'torca://pair?v=2&code=FAST82',
        ),
      ],
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Scaffold(
            body: FilledButton(
              onPressed: () => showInvitationGeneratorModal(context, gateway),
              child: const Text('Open generator'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open generator'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));

    expect(find.text('FAST82'), findsOneWidget);
    expect(
      find.bySemanticsLabel('Torca pairing invitation QR code'),
      findsOneWidget,
    );
  });

  testWidgets('creator can cancel an invitation and the modal closes', (
    WidgetTester tester,
  ) async {
    const id = '00000000000000000000000000000084';
    final gateway = FakeEngineGateway(
      responses: <FakeGatewayResponse>[
        FakeGatewayResponse.success(
          kind: 'pairing_started',
          resourceId: id,
          inviteUri: 'torca://pair?v=2&code=STOP84',
        ),
        FakeGatewayResponse.success(kind: 'pairing_cancelled'),
      ],
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Scaffold(
            body: FilledButton(
              onPressed: () => showInvitationGeneratorModal(context, gateway),
              child: const Text('Open generator'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open generator'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    await tester.tap(find.text('Cancel invitation'));
    await tester.pumpAndSettle();

    expect(find.text('Your invitation'), findsNothing);
    expect(gateway.commands.whereType<CancelPairingCommandDto>(), hasLength(1));
  });

  testWidgets(
    'generator copies the full provider invitation instead of short code',
    (WidgetTester tester) async {
      const inviteUri =
          'torca://pair?v=2&code=IROH82&provider=iroh&bootstrap=01020304';
      Object? clipboardArguments;
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async {
          if (call.method == 'Clipboard.setData') {
            clipboardArguments = call.arguments;
          }
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          SystemChannels.platform,
          null,
        ),
      );
      final gateway = FakeEngineGateway(
        responses: <FakeGatewayResponse>[
          FakeGatewayResponse.success(
            kind: 'pairing_started',
            resourceId: '00000000000000000000000000000083',
            inviteUri: inviteUri,
          ),
        ],
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) => Scaffold(
              body: FilledButton(
                onPressed: () => showInvitationGeneratorModal(context, gateway),
                child: const Text('Open generator'),
              ),
            ),
          ),
        ),
      );

      await tester.tap(find.text('Open generator'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));
      await tester.tap(find.text('Copy invitation'));
      await tester.pump();

      expect(clipboardArguments, <String, Object?>{'text': inviteUri});
      expect(find.text('Full invitation copied'), findsOneWidget);
    },
  );

  testWidgets(
    'one invitation modal follows the session state and closes on contact',
    (WidgetTester tester) async {
      const id = '00000000000000000000000000000081';
      const open = PairingDto(
        id: id,
        code: 'FLOW81',
        inviteUri: 'torca://pair?v=2&code=FLOW81',
        role: 'creator',
        state: 'open',
        expiresAtMs: 4102444800000,
        localApproved: false,
        remoteApproved: false,
      );
      final gateway = FakeEngineGateway(
        initialSnapshot: const AppSnapshotDto(
          identity: IdentityDto(displayName: 'Alice'),
          pairings: <PairingDto>[open],
          bootstrapPhase: 'ready',
        ),
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) => Scaffold(
              body: FilledButton(
                onPressed: () =>
                    showPairingSessionModal(context, gateway, open),
                child: const Text('Open invitation'),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Open invitation'));
      await tester.pump();
      expect(find.text('FLOW81'), findsOneWidget);

      gateway.publish(
        const AppSnapshotDto(
          identity: IdentityDto(displayName: 'Alice'),
          pairings: <PairingDto>[
            PairingDto(
              id: id,
              code: 'FLOW81',
              inviteUri: 'torca://pair?v=2&code=FLOW81',
              role: 'creator',
              state: 'awaiting_approval',
              expiresAtMs: 4102444800000,
              localApproved: false,
              remoteApproved: true,
              remoteDisplayName: 'Bob',
              remoteFingerprint: 'AA BB CC DD',
            ),
          ],
          bootstrapPhase: 'ready',
        ),
      );
      await tester.pump();
      expect(find.text('Bob'), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Accept'), findsOneWidget);
      expect(find.text('FLOW81'), findsNothing);

      gateway.publish(
        const AppSnapshotDto(
          identity: IdentityDto(displayName: 'Alice'),
          contacts: <ContactDto>[
            ContactDto(
              id: 'contact-81',
              displayName: 'Bob',
              status: 'active',
              connectionState: 'connecting',
            ),
          ],
          bootstrapPhase: 'ready',
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Invitation'), findsNothing);
      expect(find.text('Open invitation'), findsOneWidget);
    },
  );

  testWidgets('desktop join modal does not expose QR scanning', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => FilledButton(
            onPressed: () =>
                showJoinInvitationModal(context, FakeEngineGateway()),
            child: const Text('Open join'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open join'));
    await tester.pumpAndSettle();
    await tester.pump(const Duration(seconds: 1));

    expect(find.byType(TextField), findsOneWidget);
    expect(find.byTooltip('Scan QR'), findsNothing);
    expect(find.text('Join invitation'), findsWidgets);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('Android invitation field reconnects to the software keyboard', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => FilledButton(
            onPressed: () =>
                showJoinInvitationModal(context, FakeEngineGateway()),
            child: const Text('Open join'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open join'));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(TextField));
    await tester.pump(const Duration(milliseconds: 80));

    final field = tester.widget<TextField>(find.byType(TextField));
    expect(field.textInputAction, TextInputAction.done);
    expect(tester.testTextInput.isVisible, isTrue);
    await tester.enterText(find.byType(TextField), 'AB12CD3');
    expect(find.text('AB12CD3'), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 200));
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('profile setup is the initial recoverable route', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _app(
        FakeEngineGateway(
          initialSnapshot: const AppSnapshotDto(
            identity: IdentityDto(id: 'local-device'),
            bootstrapPhase: 'ready_for_profile',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Choose your nickname'), findsWidgets);
    expect(find.text('Your identity'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('profile-device-avatar')),
      findsOneWidget,
    );
    expect(find.byType(TorcaDeviceAvatar), findsOneWidget);
    expect(find.text('Torca'), findsOneWidget);
  });

  testWidgets('language selection precedes profile setup and persists choice', (
    WidgetTester tester,
  ) async {
    final preferences = LocalPreferences(languageChosen: false);
    await tester.pumpWidget(
      _app(
        FakeEngineGateway(
          initialSnapshot: const AppSnapshotDto(
            identity: IdentityDto(id: 'local-device'),
            bootstrapPhase: 'ready_for_profile',
          ),
        ),
        preferences: preferences,
      ),
    );

    expect(find.text('Choose your language'), findsOneWidget);
    expect(find.text('🇬🇧'), findsOneWidget);
    expect(find.text('🇵🇱'), findsOneWidget);
    expect(find.text('Choose your nickname'), findsNothing);

    await tester.tap(find.text('Polski'));
    await tester.pumpAndSettle();

    expect(preferences.languageChosen, isTrue);
    expect(preferences.localeMode, AppLocaleMode.polish);
    expect(find.text('Wybierz pseudonim'), findsWidgets);
  });

  testWidgets('settings are reachable from the shared app menu', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_app(FakeEngineGateway()));
    await tester.tap(find.byTooltip('Application menu'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Settings'));
    await tester.pumpAndSettle();

    expect(find.text('Appearance'), findsOneWidget);
    // The Appearance preview intentionally contains a representative grouped
    // conversation, so scroll to the end instead of relying on a fixed offset.
    await tester.drag(find.byType(ListView), const Offset(0, -4000));
    await tester.pumpAndSettle();
    expect(find.text('Notifications'), findsOneWidget);
    expect(find.text('Enable notifications'), findsOneWidget);
  });

  testWidgets('chat list search filters contacts and message previews', (
    WidgetTester tester,
  ) async {
    const snapshot = AppSnapshotDto(
      identity: IdentityDto(id: 'local', displayName: 'Me'),
      contacts: <ContactDto>[
        ContactDto(
          id: 'alice-contact',
          displayName: 'Alice',
          status: 'active',
          connectionState: 'ready',
        ),
        ContactDto(
          id: 'bob-contact',
          displayName: 'Bob',
          status: 'active',
          connectionState: 'ready',
        ),
      ],
      conversations: <ConversationDto>[
        ConversationDto(
          id: 'alice-conversation',
          contactId: 'alice-contact',
          status: 'active',
          lastMessageBody: 'Needle in the preview',
          lastActivityAtMs: 1,
        ),
        ConversationDto(
          id: 'bob-conversation',
          contactId: 'bob-contact',
          status: 'active',
          lastMessageBody: 'A different message',
          lastActivityAtMs: 2,
        ),
      ],
      bootstrapPhase: 'ready',
    );
    await tester.pumpWidget(_app(FakeEngineGateway(initialSnapshot: snapshot)));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Search chats'));
    await tester.pump();
    await tester.enterText(find.byType(TextField), 'needle');
    await tester.pump();

    expect(find.byTooltip('Pair contact'), findsOneWidget);
    expect(find.text('1 result'), findsOneWidget);
    expect(find.text('Alice'), findsOneWidget);
    expect(find.text('Bob'), findsNothing);
    expect(find.text('Needle in the preview'), findsOneWidget);
  });

  testWidgets(
    'active empty conversations remain visible while archived stay hidden',
    (WidgetTester tester) async {
      const snapshot = AppSnapshotDto(
        identity: IdentityDto(id: 'local', displayName: 'Me'),
        contacts: <ContactDto>[
          ContactDto(
            id: 'active-contact',
            displayName: 'Alice',
            status: 'active',
            connectionState: 'ready',
          ),
          ContactDto(
            id: 'archived-contact',
            displayName: 'Bob',
            status: 'active',
            connectionState: 'ready',
          ),
        ],
        conversations: <ConversationDto>[
          ConversationDto(
            id: 'active-empty-conversation',
            contactId: 'active-contact',
            status: 'active',
          ),
          ConversationDto(
            id: 'archived-empty-conversation',
            contactId: 'archived-contact',
            status: 'archived',
          ),
        ],
        bootstrapPhase: 'ready',
      );
      await tester.pumpWidget(
        _app(FakeEngineGateway(initialSnapshot: snapshot)),
      );
      await tester.pumpAndSettle();

      expect(find.text('Alice'), findsOneWidget);
      expect(find.text('Bob'), findsNothing);
    },
  );

  testWidgets('new contact conversation appears immediately on wide layout', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    const snapshot = AppSnapshotDto(
      identity: IdentityDto(id: 'local', displayName: 'Me'),
      contacts: <ContactDto>[
        ContactDto(
          id: 'new-contact',
          displayName: 'Alice',
          status: 'active',
          connectionState: 'ready',
        ),
      ],
      bootstrapPhase: 'ready',
    );
    final gateway = FakeEngineGateway(
      initialSnapshot: snapshot,
      responses: <FakeGatewayResponse>[
        FakeGatewayResponse.success(kind: 'attention_updated'),
        FakeGatewayResponse.success(kind: 'contacts_acknowledged'),
        FakeGatewayResponse.success(
          kind: 'conversation_started',
          resourceId: 'new-conversation',
        ),
      ],
    );
    await tester.pumpWidget(_app(gateway));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Contacts').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Alice').first);
    await tester.pumpAndSettle();

    expect(find.byType(ConversationPane), findsOneWidget);
    expect(find.textContaining('No messages yet.'), findsOneWidget);
  });

  testWidgets('warm-up renders provider-owned commissioning presentation', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _app(
        FakeEngineGateway(
          initialSnapshot: const AppSnapshotDto(
            communicationProvider: 'iroh',
            bootstrapPhase: 'starting',
            bootstrapSteps: <BootstrapStepDto>[
              BootstrapStepDto(
                id: 'communication_runtime',
                state: 'verifying',
                label: 'Iroh endpoint',
                summary: 'Binding the encrypted Iroh endpoint…',
                progress: 40,
                attempt: 1,
              ),
            ],
          ),
        ),
      ),
    );
    await tester.pump();

    expect(find.text('Iroh endpoint · attempt 1'), findsOneWidget);
    expect(find.text('Binding the encrypted Iroh endpoint…'), findsOneWidget);
    expect(find.text('40%'), findsOneWidget);
  });

  testWidgets('Android settings remain responsive while changing theme', (
    WidgetTester tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    tester.view.physicalSize = const Size(360, 900);
    tester.view.devicePixelRatio = 1;
    final preferences = LocalPreferences();
    addTearDown(() {
      debugDefaultTargetPlatformOverride = null;
      tester.view.resetPhysicalSize();
      tester.view.resetDevicePixelRatio();
      preferences.dispose();
    });

    await tester.pumpWidget(
      _app(FakeEngineGateway(), preferences: preferences),
    );
    await tester.tap(find.byTooltip('Application menu'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Settings'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Terminal'));
    await tester.pumpAndSettle();

    expect(preferences.appearance.family, TorcaThemeFamily.terminal);
    expect(find.text('Appearance'), findsOneWidget);
    expect(tester.takeException(), isNull);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets(
    'desktop settings shortcut opens and Escape dismisses the route',
    (WidgetTester tester) async {
      await tester.pumpWidget(_app(FakeEngineGateway()));

      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.comma);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pumpAndSettle();

      expect(find.text('Appearance'), findsOneWidget);

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();

      expect(find.text('Choose your nickname'), findsWidgets);
    },
  );

  testWidgets('wide layout exposes the current pairing flow', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    const profileReady = AppSnapshotDto(
      identity: IdentityDto(displayName: 'Alice'),
      transport: TransportStatusDto(
        communication: TransportIndicatorDto(state: 'ready'),
        rendezvous: TransportIndicatorDto(state: 'healthy'),
      ),
      bootstrapPhase: 'ready',
    );
    const invitationCreated = AppSnapshotDto(
      identity: IdentityDto(displayName: 'Alice'),
      transport: TransportStatusDto(
        communication: TransportIndicatorDto(state: 'ready'),
        rendezvous: TransportIndicatorDto(state: 'healthy'),
      ),
      pairings: <PairingDto>[
        PairingDto(
          id: '00000000000000000000000000000001',
          code: 'T0RCA1',
          inviteUri: 'torca://pair?v=2&code=T0RCA1',
          role: 'creator',
          state: 'open',
          expiresAtMs: 4102444800000,
          localApproved: false,
          remoteApproved: false,
        ),
      ],
      bootstrapPhase: 'ready',
    );
    await tester.pumpWidget(
      _app(
        FakeEngineGateway(
          responses: <FakeGatewayResponse>[
            FakeGatewayResponse.success(
              kind: 'profile_updated',
              snapshot: profileReady,
            ),
            FakeGatewayResponse.success(
              kind: 'pairing_started',
              resourceId: '00000000000000000000000000000001',
              snapshot: invitationCreated,
            ),
          ],
        ),
      ),
    );
    await tester.enterText(find.byType(TextField), 'Alice');
    await tester.tap(find.widgetWithText(FilledButton, 'Continue'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Invitations').first);
    await tester.pumpAndSettle();

    expect(
      find.text('Create and manage short-lived private contact invitations.'),
      findsOneWidget,
    );
    expect(
      find.widgetWithText(FilledButton, 'Generate Invitation'),
      findsOneWidget,
    );
    expect(find.text('Join an invitation'), findsNothing);
    expect(find.byType(Scaffold), findsOneWidget);
  });

  testWidgets('incoming pairing opens a global approval modal', (
    WidgetTester tester,
  ) async {
    const incoming = AppSnapshotDto(
      identity: IdentityDto(displayName: 'Alice'),
      transport: TransportStatusDto(
        communication: TransportIndicatorDto(state: 'ready'),
        rendezvous: TransportIndicatorDto(state: 'healthy'),
      ),
      pairings: <PairingDto>[
        PairingDto(
          id: '00000000000000000000000000000092',
          code: 'JOIN2',
          inviteUri: 'torca://pair?v=2&code=JOIN22',
          role: 'creator',
          state: 'peer_joined',
          expiresAtMs: 4102444800000,
          localApproved: false,
          remoteApproved: false,
          remoteIdentityId: 'remote-identity',
          remoteDisplayName: 'Bob',
          remoteFingerprint: 'AA BB CC DD',
        ),
      ],
      bootstrapPhase: 'ready',
    );
    await tester.pumpWidget(
      TorcaApp(
        gateway: FakeEngineGateway(initialSnapshot: incoming),
        navigation: AppNavigationController(),
        preferences: LocalPreferences(),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('New pairing request'), findsOneWidget);
    expect(find.text('Bob'), findsOneWidget);
    expect(find.text('AA BB CC DD'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Accept'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, 'Reject'), findsOneWidget);
  });

  testWidgets(
    'incoming approval waits for the invitation modal and appears after it closes',
    (WidgetTester tester) async {
      const pairingId = '00000000000000000000000000000093';
      final registry = PairingModalRegistry.instance;
      registry.claim(pairingId);
      addTearDown(() => registry.release(pairingId));
      const incoming = AppSnapshotDto(
        identity: IdentityDto(displayName: 'Alice'),
        transport: TransportStatusDto(
          communication: TransportIndicatorDto(state: 'ready'),
          rendezvous: TransportIndicatorDto(state: 'healthy'),
        ),
        pairings: <PairingDto>[
          PairingDto(
            id: pairingId,
            code: 'JOIN3',
            inviteUri: 'torca://pair?v=2&code=JOIN33',
            role: 'creator',
            state: 'awaiting_approval',
            expiresAtMs: 4102444800000,
            localApproved: false,
            remoteApproved: true,
            remoteIdentityId: 'remote-identity',
            remoteDisplayName: 'Carol',
            remoteFingerprint: '11 22 33 44',
          ),
        ],
        bootstrapPhase: 'ready',
      );
      await tester.pumpWidget(
        TorcaApp(
          gateway: FakeEngineGateway(initialSnapshot: incoming),
          navigation: AppNavigationController(),
          preferences: LocalPreferences(),
        ),
      );
      await tester.pump();
      await tester.pump();
      expect(find.text('New pairing request'), findsNothing);

      registry.release(pairingId);
      await tester.pumpAndSettle();

      expect(find.text('New pairing request'), findsOneWidget);
      expect(find.text('Carol'), findsOneWidget);
    },
  );

  testWidgets(
    'native startup failure is surfaced instead of silently using memory state',
    (WidgetTester tester) async {
      const String failure = 'native runtime missing';
      await tester.pumpWidget(_app(StartupFailureGateway(failure)));
      await tester.pump();

      expect(find.text(failure), findsOneWidget);
      expect(find.text('Secure runtime is not ready'), findsOneWidget);
      expect(find.text('Create local identity'), findsNothing);
    },
  );
}
