import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';

class _Gateway implements EngineGateway, AttachmentCapabilitiesProvider {
  final ValueNotifier<AppSnapshotDto> _snapshots = ValueNotifier(
    const AppSnapshotDto(),
  );

  @override
  AppCapabilities get capabilities =>
      const AppCapabilities(maxAttachmentBytes: 1234);

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
    gateway.dispose();
  });
}
