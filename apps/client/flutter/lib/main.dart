import 'package:flutter/widgets.dart';

import 'app.dart';
import 'gateway/engine_gateway.dart';
import 'gateway/memory_engine_gateway.dart';
import 'gateway/method_channel_engine_gateway.dart';

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
    final MethodChannelEngineGateway nativeGateway =
        MethodChannelEngineGateway();
    await nativeGateway.initialize();
    gateway = nativeGateway;
  }

  runApp(TorcaApp(gateway: gateway));
}
