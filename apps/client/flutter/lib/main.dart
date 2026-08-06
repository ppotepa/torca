import 'package:flutter/widgets.dart';

import 'app.dart';
import 'gateway/engine_gateway.dart';
import 'gateway/ffi_engine_gateway.dart';
import 'gateway/memory_engine_gateway.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  const bool useMemoryGateway = bool.fromEnvironment(
    'TORCA_USE_MEMORY_GATEWAY',
    defaultValue: false,
  );

  final EngineGateway gateway = useMemoryGateway
      ? MemoryEngineGateway()
      : await _openNativeGateway();

  runApp(TorcaApp(gateway: gateway));
}

Future<EngineGateway> _openNativeGateway() async {
  try {
    final FfiEngineGateway nativeGateway = FfiEngineGateway.open();
    final result = await nativeGateway.initialize();
    if (result.ok) {
      return nativeGateway;
    }

    await nativeGateway.dispose();
    return UnavailableEngineGateway(
      result.error ?? 'native Torca engine failed to initialize',
    );
  } on Object catch (error) {
    return UnavailableEngineGateway(
      'native Torca engine is unavailable: $error',
    );
  }
}
