import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/gateway/memory_engine_gateway.dart';
import 'package:torca_app/navigation/app_navigation_controller.dart';
import 'package:torca_app/settings/local_preferences.dart';

TorcaApp _app(EngineGateway gateway) => TorcaApp(
      gateway: gateway,
      navigation: AppNavigationController(),
      preferences: LocalPreferences(),
    );

void main() {
  testWidgets('identity setup is the initial recoverable route', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_app(MemoryEngineGateway()));
    expect(find.text('Create local identity'), findsWidgets);
    expect(find.text('Torca'), findsOneWidget);
  });

  testWidgets('settings are reachable from the shared app menu', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_app(MemoryEngineGateway()));
    await tester.tap(find.byTooltip('Application menu'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Settings'));
    await tester.pumpAndSettle();

    expect(find.text('Appearance'), findsOneWidget);
    expect(find.text('Notifications'), findsOneWidget);
    expect(find.text('Enable notifications'), findsOneWidget);
  });

  testWidgets('wide layout exposes the current pairing flow', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(_app(MemoryEngineGateway()));
    await tester.enterText(find.byType(TextField), 'Alice');
    await tester.tap(find.widgetWithText(FilledButton, 'Create local identity'));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Pair contact'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Create invitation'));
    await tester.pumpAndSettle();

    expect(find.text('Pairing sessions'), findsOneWidget);
    expect(find.text('TORCA1'), findsOneWidget);
    expect(find.text('Cancel'), findsOneWidget);
    expect(find.byType(Scaffold), findsOneWidget);
  });

  testWidgets(
    'native startup failure is surfaced instead of silently using memory state',
    (WidgetTester tester) async {
      const String failure = 'native runtime missing';
      await tester.pumpWidget(_app(UnavailableEngineGateway(failure)));
      await tester.enterText(find.byType(TextField), 'Alice');
      await tester.tap(find.widgetWithText(FilledButton, 'Create local identity'));
      await tester.pump();

      expect(find.text(failure), findsOneWidget);
    },
  );
}
