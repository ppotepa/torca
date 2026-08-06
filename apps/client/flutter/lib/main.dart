import 'package:flutter/widgets.dart';
import 'app.dart';
import 'gateway/memory_engine_gateway.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(TorcaApp(gateway: MemoryEngineGateway()));
}
