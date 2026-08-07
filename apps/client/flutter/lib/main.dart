import 'dart:io';

import 'package:flutter/widgets.dart';

import 'app.dart';
import 'gateway/engine_gateway.dart';
import 'gateway/ffi_engine_gateway.dart';
import 'gateway/memory_engine_gateway.dart';
import 'navigation/app_navigation_controller.dart';
import 'platform/android_notification_router.dart';
import 'platform/deep_link_router.dart';
import 'platform/desktop_lifecycle.dart';

DesktopLifecycle? _desktopLifecycle;
DeepLinkRouter? _deepLinkRouter;
AndroidNotificationRouter? _androidNotificationRouter;
AppNavigationController? _navigation;

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  const bool useMemoryGateway = bool.fromEnvironment(
    'TORCA_USE_MEMORY_GATEWAY',
    defaultValue: false,
  );

  final EngineGateway gateway = useMemoryGateway
      ? MemoryEngineGateway()
      : await _openNativeGateway();

  final navigation = AppNavigationController();
  _navigation = navigation;
  _deepLinkRouter = DeepLinkRouter(navigation);
  try {
    await _deepLinkRouter!.initialize();
  } on Object {
    // Deep links are an optional entry path; failure must not prevent secure runtime startup.
  }
  if (Platform.isAndroid) {
    _androidNotificationRouter = AndroidNotificationRouter(navigation);
    try {
      await _androidNotificationRouter!.initialize();
    } on Object {
      // Notification routing is local UI integration; messaging remains owned by RuntimeHost.
    }
  }

  if (Platform.isWindows) {
    _desktopLifecycle = DesktopLifecycle(gateway);
    await _desktopLifecycle!.initialize();
  }

  runApp(TorcaApp(gateway: gateway, navigation: navigation));
}

Future<EngineGateway> _openNativeGateway() async {
  try {
    final FfiEngineGateway nativeGateway = FfiEngineGateway.open();
    final result = await nativeGateway.initialize();
    if (result.ok) return nativeGateway;
    await nativeGateway.dispose();
    return UnavailableEngineGateway(
      result.error ?? 'native Torca engine failed to initialize',
    );
  } on Object catch (error) {
    return UnavailableEngineGateway(
      'native Torca engine is unavailable: $error',
    );
  }
}
