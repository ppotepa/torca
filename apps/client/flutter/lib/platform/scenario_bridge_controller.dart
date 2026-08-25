import '../gateway/engine_gateway.dart';

/// Control-plane capability available only from the SOAK application entrypoint.
abstract interface class ScenarioBridgeController {
  Future<void> start();

  Future<void> dispose();
}

typedef ScenarioBridgeFactory = ScenarioBridgeController Function(
  EngineGateway gateway,
);
