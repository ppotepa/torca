import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';

class AppCapabilities {
  const AppCapabilities({required this.maxAttachmentBytes});
  final int maxAttachmentBytes;
}

abstract interface class EngineGateway {
  ValueListenable<AppSnapshotDto> get snapshots;
  Future<BridgeResultDto> execute(BridgeCommandDto command);
  Future<String> diagnosticsJson();
  Future<void> dispose();
}

abstract interface class AttachmentCapabilitiesProvider {
  AppCapabilities get capabilities;
}

AppCapabilities capabilitiesFor(EngineGateway gateway) =>
    gateway is AttachmentCapabilitiesProvider
        ? gateway.capabilities
        : const AppCapabilities(maxAttachmentBytes: 16 * 1024 * 1024);

/// Optional host capability used only for an explicit application-level Quit.
abstract interface class ProcessRuntimeControl {
  Future<void> shutdown();
}

class UnavailableEngineGateway
    implements EngineGateway, ProcessRuntimeControl, AttachmentCapabilitiesProvider {
  UnavailableEngineGateway(this.reason);

  final String reason;
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto());

  @override
  AppCapabilities get capabilities =>
      const AppCapabilities(maxAttachmentBytes: 16 * 1024 * 1024);

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async => BridgeResultDto(
        ok: false,
        kind: 'error:runtime_unavailable',
        error: reason,
      );

  @override
  Future<String> diagnosticsJson() async => '{"events":[]}';

  @override
  Future<void> shutdown() async {}

  @override
  Future<void> dispose() async {
    _snapshots.dispose();
  }
}
