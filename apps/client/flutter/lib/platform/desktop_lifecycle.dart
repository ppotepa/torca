import 'dart:async';
import 'dart:io';

import 'package:local_notifier/local_notifier.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

class DesktopLifecycle with WindowListener, TrayListener {
  DesktopLifecycle(this.gateway);

  final EngineGateway gateway;
  final Set<String> _knownInbound = <String>{};
  bool _quitting = false;

  Future<void> initialize() async {
    if (!Platform.isWindows) return;
    await windowManager.ensureInitialized();
    await windowManager.setPreventClose(true);
    windowManager.addListener(this);
    trayManager.addListener(this);

    final executable = Platform.resolvedExecutable;
    final separator = Platform.pathSeparator;
    final index = executable.lastIndexOf(separator);
    final directory = index < 0 ? '.' : executable.substring(0, index);
    final trayIcon = '$directory${separator}torca.ico';
    if (File(trayIcon).existsSync()) {
      await trayManager.setIcon(trayIcon);
    }
    await trayManager.setToolTip('Torca');
    await trayManager.setContextMenu(Menu(items: <MenuItem>[
      MenuItem(key: 'show', label: 'Show Torca'),
      MenuItem.separator(),
      MenuItem(key: 'quit', label: 'Quit'),
    ]));

    await localNotifier.setup(
      appName: 'Torca',
      shortcutPolicy: ShortcutPolicy.requireCreate,
    );
    for (final message in gateway.snapshots.value.messages) {
      if (message.direction == 'inbound') _knownInbound.add(message.id);
    }
    gateway.snapshots.addListener(_snapshotChanged);
  }

  Future<void> dispose() async {
    if (!Platform.isWindows) return;
    gateway.snapshots.removeListener(_snapshotChanged);
    windowManager.removeListener(this);
    trayManager.removeListener(this);
    await trayManager.destroy();
  }

  void _snapshotChanged() {
    if (!Platform.isWindows || _quitting) return;
    final snapshot = gateway.snapshots.value;
    for (final message in snapshot.messages) {
      if (message.direction != 'inbound' || !_knownInbound.add(message.id)) continue;
      unawaited(_notify(message));
    }
  }

  Future<void> _notify(MessageDto message) async {
    if (await windowManager.isFocused()) return;
    final notification = LocalNotification(
      title: 'Torca',
      body: message.body.isEmpty ? 'New message' : message.body,
    );
    notification.onClick = () { unawaited(_showWindow()); };
    await notification.show();
  }

  Future<void> _showWindow() async {
    await windowManager.show();
    await windowManager.restore();
    await windowManager.focus();
  }

  Future<void> _quit() async {
    if (_quitting) return;
    _quitting = true;
    gateway.snapshots.removeListener(_snapshotChanged);
    await gateway.dispose();
    await trayManager.destroy();
    await windowManager.setPreventClose(false);
    await windowManager.destroy();
  }

  @override
  void onWindowClose() { if (!_quitting) unawaited(windowManager.hide()); }

  @override
  void onTrayIconMouseDown() { unawaited(_showWindow()); }

  @override
  void onTrayIconRightMouseDown() { unawaited(trayManager.popUpContextMenu()); }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    switch (menuItem.key) {
      case 'show':
        unawaited(_showWindow());
        break;
      case 'quit':
        unawaited(_quit());
        break;
    }
  }
}
