import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/screens/contact_details_screen.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_avatar/torca_avatar.dart';

void main() {
  testWidgets('identity details expose fingerprint and build metadata', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: const IdentityDetailsScreen(
          snapshot: const AppSnapshotDto(
            identity: IdentityDto(
              id: 'local-device',
              displayName: 'Alice',
              fingerprint: 'fingerprint-123',
            ),
            endpointSummary: 'endpoint-hash',
          ),
          buildInfo: const ClientBuildInfo(
            communicationProvider: 'iroh',
            productVersion: '0.3.0',
            buildId: 'build-123',
            sourceCommit: 'commit-123',
            sourceFingerprint: 'source-123',
            providerEndpointHash: null,
            providerEndpointRequired: false,
            targetPlatform: 'test',
            targetArchitecture: 'x64',
            contractSchema: 25,
            storageEpoch: 1,
            wireVersion: 1,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byType(TorcaDeviceAvatar), findsOneWidget);
    expect(find.text('fingerprint-123'), findsOneWidget);
    expect(find.text('build-123'), findsOneWidget);
    expect(find.text('commit-123'), findsOneWidget);
    expect(find.text('25 / 1'), findsOneWidget);

    final copyButton = find.text('Copy fingerprint');
    await tester.ensureVisible(copyButton);
    await tester.tap(copyButton);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text('Fingerprint copied'), findsOneWidget);
  });
}
