import 'dart:async';
import 'dart:io';

import 'package:app_links/app_links.dart';

import '../gateway/engine_gateway.dart';
import '../navigation/app_navigation_controller.dart';

class DeepLinkRouter {
  DeepLinkRouter(this.navigation, this.gateway);
  final AppNavigationController navigation;
  final EngineGateway gateway;
  final AppLinks _links = AppLinks();
  StreamSubscription<String>? _subscription;
  StreamSubscription<FileSystemEvent>? _windowsWatcher;
  bool _disposed = false;

  Future<void> initialize() async {
    if (_disposed) return;
    final initial = await _links.getInitialLink();
    if (initial != null) _accept(initial.toString());
    _subscription = _links.uriLinkStream
        .map((uri) => uri.toString())
        .listen(_accept);
    if (Platform.isWindows) {
      _watchWindowsPending();
    }
  }

  void _accept(String rawUri) {
    if (_disposed) return;
    final parser = gateway is PairingUriParser
        ? gateway as PairingUriParser
        : null;
    if (parser == null) return;
    unawaited(
      parser.parsePairingUri(rawUri).then((code) {
        if (!_disposed && code != null) navigation.openPairing(code);
      }),
    );
  }

  void _pollWindowsPending() {
    final local = Platform.environment['LOCALAPPDATA'];
    if (local == null || local.isEmpty) return;
    final file = File(
      '$local${Platform.pathSeparator}Torca${Platform.pathSeparator}pending_link.txt',
    );
    if (!file.existsSync()) return;
    try {
      final value = file.readAsStringSync().trim();
      file.deleteSync();
      _accept(value);
    } on FileSystemException {
      // A second process may still be replacing the handoff file; the next poll retries.
    }
  }

  void _watchWindowsPending() {
    final local = Platform.environment['LOCALAPPDATA'];
    if (local == null || local.isEmpty) return;
    final directory = Directory('$local${Platform.pathSeparator}Torca');
    try {
      directory.createSync(recursive: true);
      _windowsWatcher = directory
          .watch(events: FileSystemEvent.create | FileSystemEvent.modify)
          .listen((event) {
            if (event.path.endsWith('pending_link.txt')) _pollWindowsPending();
          });
      _pollWindowsPending();
    } on FileSystemException {
      // Native app activation remains authoritative. Do not fall back to a
      // periodic poll when the watcher cannot be installed.
    }
  }

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _windowsWatcher?.cancel();
    _windowsWatcher = null;
    await _subscription?.cancel();
    _subscription = null;
  }
}
