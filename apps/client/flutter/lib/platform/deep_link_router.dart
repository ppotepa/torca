import 'dart:async';
import 'dart:io';

import 'package:app_links/app_links.dart';

import '../navigation/app_navigation_controller.dart';

class DeepLinkRouter {
  DeepLinkRouter(this.navigation);
  final AppNavigationController navigation;
  final AppLinks _links = AppLinks();
  StreamSubscription<Uri>? _subscription;
  Timer? _windowsPoller;

  Future<void> initialize() async {
    final initial = await _links.getInitialLink();
    if (initial != null) _accept(initial);
    _subscription = _links.uriLinkStream.listen(_accept);
    if (Platform.isWindows) {
      _pollWindowsPending();
      _windowsPoller = Timer.periodic(const Duration(seconds: 1), (_) => _pollWindowsPending());
    }
  }

  void _accept(Uri uri) {
    if (uri.scheme != 'torca' || uri.host != 'pair' || uri.queryParameters['v'] != '1') return;
    final code = uri.queryParameters['code']?.trim().toUpperCase();
    if (code == null || !RegExp(r'^[A-Z0-9]{6,16}$').hasMatch(code)) return;
    navigation.openPairing(code);
  }

  void _pollWindowsPending() {
    final local = Platform.environment['LOCALAPPDATA'];
    if (local == null || local.isEmpty) return;
    final file = File('$local${Platform.pathSeparator}Torca${Platform.pathSeparator}0.1${Platform.pathSeparator}pending_link.txt');
    if (!file.existsSync()) return;
    try {
      final value = file.readAsStringSync().trim();
      file.deleteSync();
      final uri = Uri.tryParse(value);
      if (uri != null) _accept(uri);
    } on FileSystemException {
      // A second process may still be replacing the handoff file; the next poll retries.
    }
  }

  Future<void> dispose() async {
    _windowsPoller?.cancel();
    await _subscription?.cancel();
  }
}
