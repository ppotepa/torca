import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/diagnostics_overview.dart';

void main() {
  testWidgets('diagnostics overview renders supported health surfaces', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: const Scaffold(
          body: DiagnosticsOverview(
            diagnosticsReadable: true,
            snapshot: AppSnapshotDto(
              identity: IdentityDto(displayName: 'Alice'),
              communicationProvider: 'iroh',
              contacts: <ContactDto>[
                ContactDto(
                  id: '01',
                  displayName: 'Peer',
                  status: 'active',
                  connectionState: 'ready',
                  peerHealth: PeerHealthDto(state: 'ready', quality: 'good'),
                ),
              ],
            ),
          ),
        ),
      ),
    );

    expect(find.text('Native bridge'), findsOneWidget);
    expect(find.text('Iroh'), findsOneWidget);
    expect(find.text('Provider route'), findsOneWidget);
    expect(find.text('Direct peers'), findsOneWidget);
    expect(find.text('Diagnostics stream'), findsOneWidget);
    expect(find.text('1 of 1 direct peer links ready'), findsOneWidget);
  });
}
