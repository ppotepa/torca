import 'dart:async';

import 'package:flutter/widgets.dart';

import '../gateway/engine_gateway.dart';

/// Presentation lifecycle adapter. It never owns or recreates the process
/// runtime; it only forwards host observations to the shared Rust actor.
class RuntimeLifecycleObserver with WidgetsBindingObserver {
  RuntimeLifecycleObserver(this.gateway);
  final EngineGateway gateway;

  void attach() {
    WidgetsBinding.instance.addObserver(this);
    unawaited(gateway.sendLifecycle('host_started'));
  }

  void detach() => WidgetsBinding.instance.removeObserver(this);

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    final event = switch (state) {
      AppLifecycleState.resumed => 'foregrounded',
      AppLifecycleState.inactive ||
      AppLifecycleState.paused ||
      AppLifecycleState.hidden => 'backgrounded',
      AppLifecycleState.detached => null,
    };
    if (event != null) unawaited(gateway.sendLifecycle(event));
  }
}
