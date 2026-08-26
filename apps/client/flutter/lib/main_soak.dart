import 'package:flutter/widgets.dart';

import 'main.dart';
import 'platform/scenario_bridge.dart';

/// SOAK-only entrypoint. The production entrypoint does not import the
/// ScenarioBridge implementation.
void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(
    TorcaBootstrap(scenarioBridgeFactory: (gateway) => ScenarioBridge(gateway)),
  );
}
