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

    expect(find.text('Invitation'), findsOneWidget);
    expect(find.text('Peer joined'), findsOneWidget);
    expect(find.text('Verify'), findsOneWidget);
    expect(find.text('Approved'), findsOneWidget);
    expect(find.text('P2P ready'), findsOneWidget);
    expect(find.byIcon(Icons.check_circle), findsNWidgets(3));
  });
}
