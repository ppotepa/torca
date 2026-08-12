import 'dart:async';

import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import 'app.dart';
import 'gateway/engine_gateway.dart';
import 'gateway/ffi_engine_gateway.dart';
import 'generated/torca_contract.dart';
import 'localization/torca_strings.dart';
import 'navigation/app_navigation_controller.dart';
import 'platform/android_notification_router.dart';
import 'platform/deep_link_router.dart';
import 'platform/desktop_lifecycle.dart';
import 'platform/platform_capabilities.dart';
import 'platform/runtime_lifecycle.dart';
import 'settings/local_preferences.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const _TorcaBootstrap());
}

class _TorcaBootstrap extends StatefulWidget {
  const _TorcaBootstrap();

  @override
  State<_TorcaBootstrap> createState() => _TorcaBootstrapState();
}

class _TorcaBootstrapState extends State<_TorcaBootstrap> {
  Widget? _application;
  EngineGateway? _gateway;
  LocalPreferences? _preferences;
  AppNavigationController? _navigation;
  DesktopLifecycle? _desktopLifecycle;
  DeepLinkRouter? _deepLinkRouter;
  AndroidNotificationRouter? _androidNotificationRouter;
  bool _initializing = false;
  RuntimeLifecycleObserver? _runtimeLifecycleObserver;
  String? _startupFailure;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _initialize());
  }

  Future<void> _initialize() async {
    if (_initializing) return;
    _initializing = true;
    try {
      final preferences = LocalPreferences();
      _preferences = preferences;
      await preferences.load();
      if (!mounted) {
        await _disposeRuntimeComposition();
        return;
      }
      final EngineGateway gateway = await _openNativeGateway();
      _gateway = gateway;
      if (!mounted) {
        await _disposeRuntimeComposition();
        return;
      }
      if (gateway is FfiEngineGateway) {
        preferences.syncNotificationsEnabled(
          gateway.snapshots.value.notificationsEnabled,
        );
        preferences.syncReadReceiptsEnabled(
          gateway.snapshots.value.readReceiptsEnabled,
        );
        preferences.attachRuntimeNotificationSetting((enabled) async {
          try {
            await gateway.execute(SetNotificationsCommandDto(enabled: enabled));
          } finally {
            preferences.syncNotificationsEnabled(
              gateway.snapshots.value.notificationsEnabled,
            );
          }
        });
        preferences.attachRuntimeReadReceiptSetting((enabled) async {
          try {
            await gateway.execute(
              SetReadReceiptsEnabledCommandDto(enabled: enabled),
            );
          } finally {
            preferences.syncReadReceiptsEnabled(
              gateway.snapshots.value.readReceiptsEnabled,
            );
          }
        });
      }
      final navigation = AppNavigationController();
      _navigation = navigation;
      _deepLinkRouter = DeepLinkRouter(navigation, gateway);
      try {
        await _deepLinkRouter!.initialize();
      } on Object {
        // Deep links are an optional entry path; failure must not prevent secure runtime startup.
      }
      if (isTorcaAndroid) {
        _runtimeLifecycleObserver = RuntimeLifecycleObserver(gateway)..attach();
        _androidNotificationRouter = AndroidNotificationRouter(navigation);
        try {
          await _androidNotificationRouter!.initialize();
        } on Object {
          // Notification routing/preferences are local platform integration; messaging remains runtime-owned.
        }
      }
      if (isTorcaWindows) {
        _desktopLifecycle = DesktopLifecycle(gateway, preferences, navigation);
        await _desktopLifecycle!.initialize();
      }
      if (!mounted) {
        await _disposeRuntimeComposition();
        return;
      }
      final application = TorcaApp(
        gateway: gateway,
        navigation: navigation,
        preferences: preferences,
        onRetryBootstrap: gateway is StartupFailureGateway
            ? _retryStartup
            : null,
      );
      if (mounted) {
        setState(() {
          _startupFailure = null;
          _application = application;
        });
      }
    } on Object catch (error) {
      await _disposeRuntimeComposition();
      if (mounted) {
        setState(() {
          _application = null;
          _startupFailure = _formatNativeGatewayFailure(error);
        });
      }
    } finally {
      _initializing = false;
    }
  }

  Future<void> _retryStartup() async {
    if (_initializing || !mounted) return;
    setState(() {
      _application = null;
      _startupFailure = null;
    });
    await _disposeRuntimeComposition();
    await _initialize();
  }

  Future<void> _disposeRuntimeComposition() async {
    final runtimeLifecycleObserver = _runtimeLifecycleObserver;
    final androidNotificationRouter = _androidNotificationRouter;
    final deepLinkRouter = _deepLinkRouter;
    final desktopLifecycle = _desktopLifecycle;
    final navigation = _navigation;
    final gateway = _gateway;
    final preferences = _preferences;

    _runtimeLifecycleObserver = null;
    _androidNotificationRouter = null;
    _deepLinkRouter = null;
    _desktopLifecycle = null;
    _navigation = null;
    _gateway = null;
    _preferences = null;

    runtimeLifecycleObserver?.detach();
    androidNotificationRouter?.dispose();
    await _bestEffort(() => deepLinkRouter?.dispose() ?? Future<void>.value());
    await _bestEffort(
      () => desktopLifecycle?.dispose() ?? Future<void>.value(),
    );
    navigation?.dispose();
    preferences?.dispose();
    await _bestEffort(() => gateway?.dispose() ?? Future<void>.value());
  }

  Future<void> _bestEffort(Future<void> Function() action) async {
    try {
      await action();
    } on Object {
      // Teardown continues so one platform integration cannot leak the rest.
    }
  }

  @override
  void dispose() {
    unawaited(_disposeRuntimeComposition());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      _application ??
      _StartupScreen(failure: _startupFailure, onRetry: _retryStartup);
}

class _StartupScreen extends StatelessWidget {
  const _StartupScreen({required this.failure, required this.onRetry});

  final String? failure;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) => MaterialApp(
    debugShowCheckedModeBanner: false,
    theme: ThemeData.dark(useMaterial3: true),
    localizationsDelegates: const <LocalizationsDelegate<Object>>[
      TorcaStrings.delegate,
    ],
    supportedLocales: TorcaStrings.supportedLocales,
    home: Scaffold(
      body: SafeArea(
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              const Text(
                'Torca',
                style: TextStyle(fontSize: 32, fontWeight: FontWeight.w600),
              ),
              const SizedBox(height: 24),
              if (failure == null) ...<Widget>[
                const CircularProgressIndicator(),
                const SizedBox(height: 16),
                Text(context.strings.startingSecureNetwork),
              ] else ...<Widget>[
                Icon(TorcaIconSet.modern.error, size: 40),
                const SizedBox(height: 16),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 32),
                  child: Text(failure!, textAlign: TextAlign.center),
                ),
                const SizedBox(height: 20),
                FilledButton(
                  onPressed: onRetry,
                  child: Text(TorcaStrings.of(context).retryNow),
                ),
              ],
            ],
          ),
        ),
      ),
    ),
  );
}

Future<EngineGateway> _openNativeGateway() async {
  try {
    final FfiEngineGateway nativeGateway = await FfiEngineGateway.open();
    final result = await nativeGateway.initialize();
    if (result.ok) return nativeGateway;
    // A failed native actor is cached process-wide. Clear it before exposing
    // Retry, otherwise every retry only reacquires the same failed actor.
    await nativeGateway.shutdown();
    await nativeGateway.dispose();
    return StartupFailureGateway(
      result.error ?? 'native Torca engine failed to initialize',
    );
  } on Object catch (error) {
    return StartupFailureGateway(_formatNativeGatewayFailure(error));
  }
}

String _formatNativeGatewayFailure(Object error) {
  final detail = '$error';
  if (detail.contains('lookup symbol') ||
      detail.contains('procedure could not be found') ||
      detail.contains('Error code 127')) {
    return 'Native Torca runtime is incompatible with this application. '
        'The installed DLL/SO is from a different build. Reinstall the '
        'matching artifact; the deployment manifest will show the build id.\n\n'
        'Diagnostic: $detail';
  }
  return 'native Torca engine is unavailable: $detail';
}
