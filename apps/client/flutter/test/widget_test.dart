import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/gateway/memory_engine_gateway.dart';
import 'package:torca_app/navigation/app_navigation_controller.dart';
import 'package:torca_app/settings/local_preferences.dart';

void main() {
  testWidgets('Torca app renders its identity setup route', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(TorcaApp(
      gateway: MemoryEngineGateway(),
      navigation: AppNavigationController(),
      preferences: LocalPreferences(),
    ));

    expect(find.text('Torca'), findsOneWidget);
    expect(find.text('Create local identity'), findsWidgets);
  });
}
