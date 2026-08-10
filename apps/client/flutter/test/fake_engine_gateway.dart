import 'dart:collection';

import 'package:flutter/foundation.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';

/// A deliberately passive gateway for widget tests.
///
/// Tests provide the snapshots and responses which the native runtime would
/// have produced. This fake never interprets commands, validates user input,
/// generates identifiers, or mutates domain state.
class FakeEngineGateway implements EngineGateway, PairingUriParser {
  FakeEngineGateway({
    AppSnapshotDto initialSnapshot = _readyWithoutProfile,
    Iterable<FakeGatewayResponse> responses = const <FakeGatewayResponse>[],
    Iterable<String?> parsedPairingCodes = const <String?>[],
    this.diagnostics = '{"events":[]}',
  }) : _snapshots = ValueNotifier<AppSnapshotDto>(initialSnapshot),
       _responses = ListQueue<FakeGatewayResponse>.of(responses),
       _parsedPairingCodes = ListQueue<String?>.of(parsedPairingCodes);

  static const AppSnapshotDto _readyWithoutProfile = AppSnapshotDto(
    torState: 'ready',
    transport: TransportStatusDto(
      tor: TransportIndicatorDto(state: 'ready'),
      relay: TransportIndicatorDto(state: 'healthy'),
    ),
    bootstrapPhase: 'ready',
  );

  final ValueNotifier<AppSnapshotDto> _snapshots;
  final ListQueue<FakeGatewayResponse> _responses;
  final ListQueue<String?> _parsedPairingCodes;
  final String diagnostics;

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Stream<RuntimeEventDto> get events => const Stream<RuntimeEventDto>.empty();

  @override
  Future<void> sendLifecycle(String event) async {}

  /// Publishes a snapshot explicitly, for example for a simulated push event.
  void publish(AppSnapshotDto snapshot) => _snapshots.value = snapshot;

  @override
  Future<String?> parsePairingUri(String rawUri) async =>
      _parsedPairingCodes.isEmpty ? null : _parsedPairingCodes.removeFirst();

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    if (_responses.isEmpty) {
      return const BridgeResultDto(ok: true, kind: 'snapshot');
    }
    final response = _responses.removeFirst();
    if (response.snapshot != null) publish(response.snapshot!);
    return response.result;
  }

  @override
  Future<String> diagnosticsJson() async => diagnostics;

  @override
  Future<void> dispose() async {
    _snapshots.dispose();
  }
}

class FakeGatewayResponse {
  const FakeGatewayResponse({required this.result, this.snapshot});

  FakeGatewayResponse.success({
    required String kind,
    String? resourceId,
    this.snapshot,
  }) : result = BridgeResultDto(ok: true, kind: kind, resourceId: resourceId);

  final BridgeResultDto result;
  final AppSnapshotDto? snapshot;
}
