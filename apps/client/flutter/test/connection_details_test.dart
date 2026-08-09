import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/screens/connection_details_screen.dart';
import 'package:torca_app/theme/app_theme.dart';

class _SnapshotGateway implements EngineGateway {
  _SnapshotGateway(AppSnapshotDto snapshot)
    : _snapshot = ValueNotifier(snapshot);
  final ValueNotifier<AppSnapshotDto> _snapshot;
  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshot;
  @override
  Stream<RuntimeEventDto> get events => const Stream<RuntimeEventDto>.empty();
  @override
  Future<void> sendLifecycle(String event) async {}
  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async =>
      const BridgeResultDto(ok: true, kind: 'noop');
  @override
  Future<String> diagnosticsJson() async => '{"events":[]}';
  @override
  Future<void> dispose() async => _snapshot.dispose();
}

void main() {
  testWidgets('connection details render runtime-owned peer health', (
    tester,
  ) async {
    final gateway = _SnapshotGateway(
      const AppSnapshotDto(
        contacts: <ContactDto>[
          ContactDto(
            id: '01',
            displayName: 'Alice',
            onionAddress: 'alice.onion',
            status: 'active',
            connectionState: 'ready',
            peerHealth: PeerHealthDto(
              state: 'ready',
              quality: 'good',
              rttMs: 721,
              lastSuccessAtMs: 1000,
              reconnectAttempt: 1,
            ),
          ),
        ],
      ),
    );
    addTearDown(gateway.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: ConnectionDetailsScreen(gateway: gateway, contactId: '01'),
      ),
    );

    expect(find.text('Alice'), findsOneWidget);
    expect(find.text('Good'), findsWidgets);
    expect(find.text('721 ms'), findsWidgets);
    expect(find.text('Direct P2P over Tor'), findsOneWidget);
    expect(find.text('1'), findsOneWidget);
  });
}
