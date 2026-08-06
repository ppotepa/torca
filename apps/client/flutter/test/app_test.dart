import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/gateway/memory_engine_gateway.dart';

void main() {
  testWidgets(
    'identity setup is the initial recoverable route',
    (WidgetTester tester) async {
      await tester.pumpWidget(TorcaApp(gateway: MemoryEngineGateway()));
      expect(find.text('Create local identity'), findsWidgets);
      expect(find.text('Torca'), findsOneWidget);
    },
  );

  testWidgets(
    'wide layout reuses the conversation pane instead of a desktop implementation',
    (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(TorcaApp(gateway: MemoryEngineGateway()));
      await tester.enterText(find.byType(TextField), 'Alice');
      await tester.tap(find.widgetWithText(FilledButton, 'Create local identity'));
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('Pair contact'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Start pairing'));
      await tester.pumpAndSettle();

      expect(find.text('No messages yet'), findsOneWidget);
      expect(find.byIcon(Icons.person_outline), findsOneWidget);
      expect(find.byType(Scaffold), findsOneWidget);
    },
  );

  testWidgets(
    'native startup failure is surfaced instead of silently using memory state',
    (WidgetTester tester) async {
      const String failure = 'native runtime missing';
      await tester.pumpWidget(
        TorcaApp(gateway: UnavailableEngineGateway(failure)),
      );
      await tester.enterText(find.byType(TextField), 'Alice');
      await tester.tap(find.widgetWithText(FilledButton, 'Create local identity'));
      await tester.pump();

      expect(find.text(failure), findsOneWidget);
    },
  );
}
