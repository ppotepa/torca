import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/navigation/app_navigation_controller.dart';
import 'package:torca_app/settings/local_preferences.dart';
import 'fake_engine_gateway.dart';

TorcaApp _app(EngineGateway gateway) => TorcaApp(
  gateway: gateway,
  navigation: AppNavigationController(),
  preferences: LocalPreferences(),
);

void main() {
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
              snapshot: invitationCreated,
            ),
          ],
        ),
      ),
    );
    await tester.enterText(find.byType(TextField), 'Alice');
    await tester.tap(find.widgetWithText(FilledButton, 'Continue'));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Pair contact'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Create invitation'));
    await tester.pumpAndSettle();

    expect(find.text('Active invitations'), findsOneWidget);
    expect(find.text('T0R-CA1'), findsOneWidget);
    expect(find.text('Cancel invitation'), findsOneWidget);
    expect(find.byType(Scaffold), findsOneWidget);
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
