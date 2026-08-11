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
import 'fake_engine_gateway.dart';

TorcaApp _app(EngineGateway gateway) => TorcaApp(
  gateway: gateway,
  navigation: AppNavigationController(),
  preferences: LocalPreferences(),
);

void main() {
  testWidgets('invitations screen only exposes invitation generation', (
    WidgetTester tester,
  ) async {
    const snapshot = AppSnapshotDto(
      identity: IdentityDto(displayName: 'Alice'),
      torState: 'ready',
      transport: TransportStatusDto(
        tor: TransportIndicatorDto(state: 'ready'),
        relay: TransportIndicatorDto(state: 'degraded'),
      ),
      bootstrapPhase: 'ready',
    );
    await tester.pumpWidget(
      MaterialApp(
        home: PairingScreen(
          gateway: FakeEngineGateway(initialSnapshot: snapshot),
        ),
      ),
    );
    expect(find.text('Join an invitation'), findsNothing);
    expect(find.byType(TextField), findsNothing);
    expect(
      find.widgetWithText(FilledButton, 'Generate Invitation'),
      findsOneWidget,
    );
  });

  testWidgets('queued pairing remains visible while network recovers', (
    WidgetTester tester,
  ) async {
    const snapshot = AppSnapshotDto(
      identity: IdentityDto(displayName: 'Alice'),
      bootstrapPhase: 'ready',
      pendingOperations: <PendingOperationDto>[
        PendingOperationDto(
          id: '01',
          resourceId: '02',
          kind: 'pairing.create',
          state: 'retrying',
          dependency: 'relay',
          attempts: 2,
          nextAttemptAtMs: 100,
          createdAtMs: 1,
        ),
      ],
    );

    await tester.pumpWidget(
      MaterialApp(
        home: PairingScreen(
          gateway: FakeEngineGateway(initialSnapshot: snapshot),
        ),
      ),
    );

    expect(find.text('Waiting for network'), findsOneWidget);
    expect(find.text('Generating invitation'), findsOneWidget);
    expect(find.text('Retry 2 · waiting for secure relay'), findsOneWidget);
  });

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

    expect(find.byType(TextField), findsOneWidget);
    expect(find.byTooltip('Scan QR'), findsNothing);
    expect(find.text('Join invitation'), findsWidgets);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('profile setup is the initial recoverable route', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_app(FakeEngineGateway()));
    expect(find.text('Choose your nickname'), findsWidgets);
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
          id: '00000000000000000000000000000002',
          code: 'JOIN2',
          inviteUri: 'torca://pair?v=2&code=JOIN22',
          role: 'creator',
          state: 'peerjoined',
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
    await tester.pump();
    await tester.pump();

    expect(find.text('New pairing request'), findsOneWidget);
    expect(find.text('Bob'), findsOneWidget);
    expect(find.text('AA BB CC DD'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Accept'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, 'Reject'), findsOneWidget);
  });

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
