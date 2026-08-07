import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/gateway/memory_engine_gateway.dart';

void main() {
  testWidgets('Torca app renders its identity setup route', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(TorcaApp(gateway: MemoryEngineGateway()));

    expect(find.text('Torca'), findsOneWidget);
    expect(find.text('Create local identity'), findsWidgets);
  });
}
