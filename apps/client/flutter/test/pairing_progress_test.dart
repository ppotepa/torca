import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/pairing_progress.dart';

void main() {
  testWidgets('pairing progress reflects runtime stage', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: const Scaffold(body: PairingProgress(state: 'awaiting_approval')),
      ),
    );

    expect(find.byIcon(Icons.qr_code_2), findsOneWidget);
    expect(find.byIcon(Icons.link), findsOneWidget);
    expect(find.byIcon(Icons.verified_user_outlined), findsOneWidget);
    expect(find.byIcon(Icons.check_circle_outline), findsOneWidget);
    expect(find.byIcon(Icons.hub_outlined), findsOneWidget);
    expect(find.byIcon(Icons.arrow_forward), findsNWidgets(4));
  });
}
