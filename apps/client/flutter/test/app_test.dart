import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/main.dart';

void main() {
  testWidgets('renders the foundation shell', (WidgetTester tester) async {
    await tester.pumpWidget(const TorcaApp());

    expect(find.text('Torca 0.1'), findsOneWidget);
    expect(find.text('Foundation workspace'), findsOneWidget);
  });
}
