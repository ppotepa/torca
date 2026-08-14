import 'dart:async';

import 'package:flutter/widgets.dart';

import '../gateway/engine_gateway.dart';

/// Presentation lifecycle adapter. It never owns or recreates the process
/// runtime; it only forwards host observations to the shared Rust actor.
class RuntimeLifecycleObserver with WidgetsBindingObserver {
  RuntimeLifecycleObserver(this.gateway);
  final EngineGateway gateway;
  String? _lastEvent;

  void attach() {
    WidgetsBinding.instance.addObserver(this);
    _lastEvent = 'host_started';
    unawaited(gateway.sendLifecycle('host_started'));
  }

  void detach() => WidgetsBinding.instance.removeObserver(this);

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    final event = switch (state) {
      AppLifecycleState.resumed => 'foregrounded',
      // Android emits inactive for permission sheets, the keyboard and other
      // temporary focus changes. Treating it as background interrupts PTT and
      // destabilises streams while the activity is still visible.
      AppLifecycleState.inactive => null,
      AppLifecycleState.paused || AppLifecycleState.hidden => 'backgrounded',
      AppLifecycleState.detached => null,
    };
    if (event == null || event == _lastEvent) return;
    _lastEvent = event;
    unawaited(gateway.sendLifecycle(event));
  }
}
