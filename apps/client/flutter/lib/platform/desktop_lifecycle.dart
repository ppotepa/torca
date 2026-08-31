import 'dart:async';
import 'dart:io';

import 'package:local_notifier/local_notifier.dart';
import 'package:torca_avatar/torca_avatar.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../navigation/app_navigation_controller.dart';
import '../settings/local_preferences.dart';

class DesktopLifecycle with WindowListener, TrayListener {
  DesktopLifecycle(this.gateway, this.preferences, this.navigation);
  final EngineGateway gateway;
  final LocalPreferences preferences;
  final AppNavigationController navigation;
  StreamSubscription<RuntimeEventDto>? _eventSubscription;
  bool _quitting = false;
  bool _disposed = false;
  bool _windowBackgrounded = false;
  Timer? _trayUpdateTimer;
  String? _lastTraySignature;

  Future<void> initialize() async {
    if (!Platform.isWindows || _disposed) return;
    await windowManager.ensureInitialized();
    await windowManager.setPreventClose(true);
    // A tray-launched process may already be minimized before the first
    // window event reaches Flutter. Seed the shared avatar clock from the
    // actual window state so it never starts an animation ticker in that case.
    final initiallyMinimized = await windowManager.isMinimized();
    _windowBackgrounded = initiallyMinimized;
    AvatarFrameClock.instance.setWindowVisible(!initiallyMinimized);
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
    if (initiallyMinimized) {
      unawaited(gateway.sendLifecycle('backgrounded'));
    }
  }

  Future<void> dispose() async {
    if (!Platform.isWindows || _disposed) return;
    _disposed = true;
    _trayUpdateTimer?.cancel();
    _trayUpdateTimer = null;
    await _eventSubscription?.cancel();
    _eventSubscription = null;
    preferences.removeListener(_preferencesChanged);
    windowManager.removeListener(this);
    trayManager.removeListener(this);
    await trayManager.destroy();
  }

  @override
  void onWindowMinimize() {
    AvatarFrameClock.instance.setWindowVisible(false);
    if (!_windowBackgrounded && !_quitting && !_disposed) {
      _windowBackgrounded = true;
      unawaited(gateway.sendLifecycle('backgrounded'));
    }
  }

  @override
  void onWindowRestore() {
    AvatarFrameClock.instance.setWindowVisible(true);
    if (_windowBackgrounded && !_quitting && !_disposed) {
      _windowBackgrounded = false;
      unawaited(gateway.sendLifecycle('foregrounded'));
    }
  }

  void _preferencesChanged() {
    _scheduleTrayMenuUpdate();
  }

  void _runtimeEvent(RuntimeEventDto event) {
    if (!Platform.isWindows || _quitting || _disposed) return;
    _scheduleTrayMenuUpdate();
    if (preferences.notificationsEnabled) unawaited(_notify(event));
  }

  void _scheduleTrayMenuUpdate() {
    if (!Platform.isWindows || _quitting || _disposed) return;
    _trayUpdateTimer ??= Timer(const Duration(milliseconds: 250), () {
      _trayUpdateTimer = null;
      unawaited(_updateTrayMenu());
    });
  }

  Future<void> _updateTrayMenu() async {
    if (!Platform.isWindows || _quitting || _disposed) return;
    final snapshot = gateway.snapshots.value;
    final readyPeers = snapshot.contacts
        .where(
          (contact) => contact.peerHealth.typedState == TransportState.ready,
        )
        .length;
    final signature =
        '${snapshot.communicationProvider}|${snapshot.communicationState}|$readyPeers';
    if (signature == _lastTraySignature) return;
    _lastTraySignature = signature;
    await trayManager.setContextMenu(
      Menu(
        items: <MenuItem>[
          MenuItem(key: 'show', label: 'Show Torca'),
          MenuItem(
            key: 'provider-status',
            label:
                '${snapshot.communicationProvider.toUpperCase()}: ${_communicationLabel(snapshot.communicationState)}',
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

  Future<void> _notify(RuntimeEventDto event) async {
    if (!preferences.notificationsEnabled || await windowManager.isFocused())
      return;
    if (event.conversationId.isNotEmpty &&
        await preferences.conversationMuted(event.conversationId)) {
      return;
    }
    final notification = LocalNotification(
      title: event.title,
      body: event.body,
      actions: switch (event.kind) {
        'message_received' => <LocalNotificationAction>[
          LocalNotificationAction(text: 'Mark read'),
        ],
        'pairing_request' => <LocalNotificationAction>[
          LocalNotificationAction(text: 'Accept'),
          LocalNotificationAction(text: 'Reject'),
        ],
        _ => null,
      },
    );
    notification.onClick = () {
      if (event.conversationId.isNotEmpty) {
        navigation.openConversation(event.conversationId);
      } else if (event.kind == 'pairing_request' &&
          event.resourceId.isNotEmpty) {
        navigation.openPairingSession(event.resourceId);
      }
      unawaited(_showWindow());
    };
    notification.onClickAction = (index) {
      unawaited(_handleNotificationAction(event, index));
    };
    await notification.show();
  }

  Future<void> _handleNotificationAction(
    RuntimeEventDto event,
    int index,
  ) async {
    if (event.kind == 'message_received' &&
        index == 0 &&
        event.conversationId.isNotEmpty) {
      await gateway.execute(
        MarkConversationReadCommandDto(conversationIdHex: event.conversationId),
      );
    } else if (event.kind == 'pairing_request' && event.resourceId.isNotEmpty) {
      if (index == 0) {
        await gateway.execute(
          ApprovePairingCommandDto(sessionIdHex: event.resourceId),
        );
      } else if (index == 1) {
        await gateway.execute(
          RejectPairingCommandDto(sessionIdHex: event.resourceId),
        );
      }
      navigation.openPairingSession(event.resourceId);
    }
    await _showWindow();
  }

  Future<void> _showWindow() async {
    // Flutter's desktop lifecycle does not reliably emit `resumed` for a
    // window restored from the tray. This is a host visibility fact, not a
    // second lifecycle owner; it complements RuntimeLifecycleObserver.
    _windowBackgrounded = false;
    await gateway.sendLifecycle('foregrounded');
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

  String _communicationLabel(String state) => switch (state) {
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
      if (!_windowBackgrounded) {
        _windowBackgrounded = true;
        unawaited(gateway.sendLifecycle('backgrounded'));
      }
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
