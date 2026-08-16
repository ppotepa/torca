import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/transfer_center.dart';

class _Gateway implements EngineGateway {
  final ValueNotifier<AppSnapshotDto> _snapshots = ValueNotifier(
    const AppSnapshotDto(),
  );

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Stream<RuntimeEventDto> get events => const Stream<RuntimeEventDto>.empty();

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async =>
      const BridgeResultDto(ok: true, kind: 'accepted');

  @override
  Future<void> sendLifecycle(String event) async {}

  @override
  Future<String> diagnosticsJson() async => '{"events":[]}';

  @override
  Future<void> dispose() async => _snapshots.dispose();
}

void main() {
  testWidgets('transfer center filters recordings independently of media', (
    tester,
  ) async {
    final gateway = _Gateway();
    addTearDown(gateway.dispose);
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: TransferCenterButton(
            gateway: gateway,
            pendingOperations: const <PendingOperationDto>[],
            attachments: const <AttachmentDto>[
              AttachmentDto(
                id: 'voice',
                messageId: 'message-1',
                name: 'voice.m4a',
                mediaType: 'audio/mp4',
                size: 100,
                status: 'transferring',
                offset: 50,
              ),
              AttachmentDto(
                id: 'photo',
                messageId: 'message-2',
                name: 'photo.jpg',
                mediaType: 'image/jpeg',
                size: 100,
                status: 'transferring',
                offset: 25,
              ),
            ],
          ),
        ),
      ),
    );

    await tester.tap(find.byTooltip('Transfers'));
    await tester.pumpAndSettle();
    expect(find.text('voice.m4a'), findsOneWidget);
    expect(find.text('photo.jpg'), findsOneWidget);

    await tester.tap(find.widgetWithText(FilterChip, 'Recordings'));
    await tester.pumpAndSettle();
    expect(find.text('voice.m4a'), findsOneWidget);
    expect(find.text('photo.jpg'), findsNothing);
  });
}
