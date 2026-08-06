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

  final EngineGateway gateway;
  if (useMemoryGateway) {
    gateway = MemoryEngineGateway();
  } else {
    try {
      final FfiEngineGateway nativeGateway = FfiEngineGateway.open();
      final result = await nativeGateway.initialize();
      gateway = result.ok
          ? nativeGateway
          : UnavailableEngineGateway(
              result.error ?? 'native Torca engine failed to initialize',
            );
      if (!result.ok) {
        await nativeGateway.dispose();
      }
    } on Object catch (error) {
      gateway = UnavailableEngineGateway(
        'native Torca engine is unavailable: $error',
      );
    }
  }

  runApp(TorcaApp(gateway: gateway));
}
