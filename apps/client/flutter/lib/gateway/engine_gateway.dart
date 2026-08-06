import 'package:flutter/foundation.dart';
import '../generated/torca_contract.dart';

abstract interface class EngineGateway {
  ValueListenable<AppSnapshotDto> get snapshots;
  Future<BridgeResultDto> execute(BridgeCommandDto command);
  Future<void> dispose();
}
