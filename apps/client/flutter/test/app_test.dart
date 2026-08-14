import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/navigation/app_navigation_controller.dart';
import 'package:torca_app/screens/pairing_screen.dart';
import 'package:torca_app/settings/local_preferences.dart';
import 'package:torca_app/widgets/pairing_modal_registry.dart';
import 'package:torca_avatar/torca_avatar.dart';
import 'fake_engine_gateway.dart';

TorcaApp _app(EngineGateway gateway) => TorcaApp(
  gateway: gateway,
  navigation: AppNavigationController(),
  preferences: LocalPreferences(),
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
              onionAddress: 'bob.onion:443',
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
            torState: 'ready',
            bootstrapPhase: 'ready_for_profile',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Choose your nickname'), findsWidgets);
    expect(find.text('THIS IS YOUR UGLY FACE'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('profile-device-avatar')),
      findsOneWidget,
    );
    expect(find.byType(TorcaDeviceAvatar), findsOneWidget);
    expect(find.text('Torca'), findsOneWidget);
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
    expect(find.text('Notifications'), findsOneWidget);
    expect(find.text('Enable notifications'), findsOneWidget);
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
      torState: 'ready',
      transport: TransportStatusDto(
        tor: TransportIndicatorDto(state: 'ready'),
        relay: TransportIndicatorDto(state: 'healthy'),
      ),
      bootstrapPhase: 'ready',
    );
    const invitationCreated = AppSnapshotDto(
      identity: IdentityDto(displayName: 'Alice'),
      torState: 'ready',
      transport: TransportStatusDto(
        tor: TransportIndicatorDto(state: 'ready'),
        relay: TransportIndicatorDto(state: 'healthy'),
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
      torState: 'ready',
      transport: TransportStatusDto(
        tor: TransportIndicatorDto(state: 'ready'),
        relay: TransportIndicatorDto(state: 'healthy'),
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
        torState: 'ready',
        transport: TransportStatusDto(
          tor: TransportIndicatorDto(state: 'ready'),
          relay: TransportIndicatorDto(state: 'healthy'),
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
