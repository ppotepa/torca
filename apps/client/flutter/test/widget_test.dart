import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/app.dart';
import 'package:torca_app/navigation/app_navigation_controller.dart';
import 'package:torca_app/settings/local_preferences.dart';
import 'fake_engine_gateway.dart';

void main() {
  testWidgets('Torca app renders its profile setup route', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      TorcaApp(
        gateway: FakeEngineGateway(),
        navigation: AppNavigationController(),
        preferences: LocalPreferences(),
      ),
    );

    expect(find.text('Torca'), findsOneWidget);
    expect(find.text('Choose your nickname'), findsWidgets);
  });
}
