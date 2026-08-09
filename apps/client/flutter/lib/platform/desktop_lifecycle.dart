import 'dart:async';
import 'dart:io';

import 'package:local_notifier/local_notifier.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import '../gateway/engine_gateway.dart';
import '../navigation/app_navigation_controller.dart';
import '../settings/local_preferences.dart';

class DesktopLifecycle with WindowListener, TrayListener {
  DesktopLifecycle(this.gateway, this.preferences, this.navigation);
  final EngineGateway gateway;
  final LocalPreferences preferences;
  final AppNavigationController navigation;
  StreamSubscription<RuntimeEventDto>? _eventSubscription;
  bool _quitting = false;

  Future<void> initialize() async {
    if (!Platform.isWindows) return;
    await gateway.sendLifecycle('host_started');
    await windowManager.ensureInitialized();
    await windowManager.setPreventClose(true);
    windowManager.addListener(this);
    trayManager.addListener(this);
    final executable = Platform.resolvedExecutable;
    final separator = Platform.pathSeparator;
    final index = executable.lastIndexOf(separator);
    final directory = index < 0 ? '.' : executable.substring(0, index);
    final trayIcon = '$directory${separator}torca.ico';
    if (File(trayIcon).existsSync()) await trayManager.setIcon(trayIcon);
    await trayManager.setToolTip('Torca');
    await _updateTrayMenu();
    await localNotifier.setup(
      appName: 'Torca',
      shortcutPolicy: ShortcutPolicy.requireCreate,
    );
    _eventSubscription = gateway.events.listen(_runtimeEvent);
    preferences.addListener(_preferencesChanged);
  }

  Future<void> dispose() async {
    if (!Platform.isWindows) return;
    await _eventSubscription?.cancel();
    preferences.removeListener(_preferencesChanged);
    windowManager.removeListener(this);
    trayManager.removeListener(this);
    await trayManager.destroy();
  }

  void _preferencesChanged() {
    if (!_quitting) unawaited(_updateTrayMenu());
  }

  void _runtimeEvent(RuntimeEventDto event) {
    if (!Platform.isWindows || _quitting) return;
    unawaited(_updateTrayMenu());
    if (preferences.notificationsEnabled) unawaited(_notify());
  }

  Future<void> _updateTrayMenu() async {
    if (!Platform.isWindows || _quitting) return;
    final snapshot = gateway.snapshots.value;
    final readyPeers = snapshot.contacts
        .where((contact) => contact.peerHealth.state == 'ready')
        .length;
    await trayManager.setContextMenu(
      Menu(
        items: <MenuItem>[
          MenuItem(key: 'show', label: 'Show Torca'),
          MenuItem(
            key: 'tor-status',
            label: 'Tor: ${_torLabel(snapshot.torState)}',
          ),
          MenuItem(key: 'peer-status', label: 'Peers: $readyPeers connected'),
          MenuItem.separator(),
          MenuItem(key: 'pair', label: 'New pairing'),
          MenuItem.separator(),
          MenuItem(key: 'quit', label: 'Quit'),
        ],
      ),
    );
  }

  Future<void> _notify() async {
    if (!preferences.notificationsEnabled || await windowManager.isFocused())
      return;
    final notification = LocalNotification(
      title: 'Torca',
      body: 'New private message',
    );
    notification.onClick = () {
      unawaited(_showWindow());
    };
    await notification.show();
  }

  Future<void> _showWindow() async {
    await windowManager.show();
    await windowManager.restore();
    await windowManager.focus();
  }

  Future<void> _newPairing() async {
    navigation.requestNewPairing();
    await _showWindow();
  }

  Future<void> _quit() async {
    if (_quitting) return;
    _quitting = true;
    await _eventSubscription?.cancel();
    preferences.removeListener(_preferencesChanged);
    try {
      await gateway.sendLifecycle('terminating');
      if (gateway is RuntimeShutdownGateway) {
        await (gateway as RuntimeShutdownGateway).shutdown();
      }
    } finally {
      await gateway.dispose();
      await trayManager.destroy();
      await windowManager.setPreventClose(false);
      await windowManager.destroy();
    }
  }

  String _torLabel(String state) => switch (state) {
    'ready' => 'Connected',
    'starting' => 'Starting',
    'reconnecting' => 'Reconnecting',
    'failed' => 'Failed',
    _ => state,
  };

  @override
  void onWindowClose() {
    if (_quitting) return;
    if (preferences.closeToTrayEnabled) {
      unawaited(gateway.sendLifecycle('backgrounded'));
      unawaited(windowManager.hide());
    } else {
      unawaited(_quit());
    }
  }

  @override
  void onTrayIconMouseDown() {
    unawaited(_showWindow());
  }

  @override
  void onTrayIconRightMouseDown() {
    unawaited(trayManager.popUpContextMenu());
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    switch (menuItem.key) {
      case 'show':
        unawaited(_showWindow());
      case 'pair':
        unawaited(_newPairing());
      case 'quit':
        unawaited(_quit());
    }
  }
}
