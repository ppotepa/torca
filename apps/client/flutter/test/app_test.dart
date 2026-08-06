import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/gateway/memory_engine_gateway.dart';

void main() {
  testWidgets('identity setup is the initial recoverable route', (tester) async {
    await tester.pumpWidget(TorcaApp(gateway: MemoryEngineGateway()));
    expect(find.text('Create local identity'), findsOneWidget);
    expect(find.text('Torca'), findsOneWidget);
  });
}
