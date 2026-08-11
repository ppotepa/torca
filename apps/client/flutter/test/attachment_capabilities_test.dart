import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';

class _Gateway implements EngineGateway, AttachmentCapabilitiesProvider {
  final ValueNotifier<AppSnapshotDto> _snapshots = ValueNotifier(
    const AppSnapshotDto(),
  );

  @override
  ClientCapabilitiesDto get capabilities => const ClientCapabilitiesDto(
    maxAttachmentBytes: 1234,
    maxVideoAttachmentBytes: 4321,
    maxQueuedAttachments: 3,
    maxAttachmentSourceBytes: 9876,
  );

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Stream<RuntimeEventDto> get events => const Stream<RuntimeEventDto>.empty();
  @override
  Future<void> sendLifecycle(String event) async {}

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async =>
      const BridgeResultDto(ok: true, kind: 'ok');

  @override
  Future<String> diagnosticsJson() async => '{}';

  @override
  Future<void> dispose() async => _snapshots.dispose();
}

void main() {
  test('attachment capability is obtained through the gateway boundary', () {
    final gateway = _Gateway();
    expect(capabilitiesFor(gateway).maxAttachmentBytes, 1234);
    expect(capabilitiesFor(gateway).maxVideoAttachmentBytes, 4321);
    expect(capabilitiesFor(gateway).maxQueuedAttachments, 3);
    gateway.dispose();
  });

  test('native metadata exposes one typed capability set', () {
    final info = ClientBuildInfo.fromJson(<String, dynamic>{
      'capabilities': <String, dynamic>{
        'maxAttachmentBytes': 10,
        'maxVideoAttachmentBytes': 7,
        'maxQueuedAttachments': 2,
        'maxAttachmentSourceBytes': 20,
      },
    });
    expect(info.capabilities.maxAttachmentBytes, 10);
    expect(info.capabilities.maxVideoAttachmentBytes, 7);
    expect(info.capabilities.maxQueuedAttachments, 2);
    expect(info.capabilities.maxAttachmentSourceBytes, 20);
  });
}
