import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/pairing_progress.dart';
import 'package:torca_ui/torca_ui.dart';

void main() {
  testWidgets('pairing progress reflects runtime stage', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: const Scaffold(body: PairingProgress(state: 'awaiting_approval')),
      ),
    );

    expect(find.byIcon(TorcaIconSet.modern.invitations), findsOneWidget);
    expect(find.byIcon(TorcaIconSet.modern.link), findsOneWidget);
    expect(find.byIcon(TorcaIconSet.modern.identity), findsOneWidget);
    expect(find.byIcon(TorcaIconSet.modern.confirm), findsOneWidget);
    expect(find.byIcon(TorcaIconSet.modern.online), findsOneWidget);
    expect(find.byIcon(TorcaIconSet.modern.send), findsNWidgets(4));
  });
}
