import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';

abstract interface class EngineGateway {
  ValueListenable<AppSnapshotDto> get snapshots;
  Future<BridgeResultDto> execute(BridgeCommandDto command);
  Future<String> diagnosticsJson();
  Future<void> dispose();
}

class UnavailableEngineGateway implements EngineGateway {
  UnavailableEngineGateway(this.reason);

  final String reason;
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto());

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    return BridgeResultDto(ok: false, kind: 'error', error: reason);
  }

  @override
  Future<String> diagnosticsJson() async => '{"events":[]}';

  @override
  Future<void> dispose() async {
    _snapshots.dispose();
  }
}
