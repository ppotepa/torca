import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'app.dart';
import 'gateway/engine_gateway.dart';
import 'gateway/ffi_engine_gateway.dart';
import 'generated/torca_contract.dart';
import 'navigation/app_navigation_controller.dart';
import 'platform/android_notification_router.dart';
import 'platform/deep_link_router.dart';
import 'platform/desktop_lifecycle.dart';
import 'platform/platform_capabilities.dart';
import 'platform/runtime_lifecycle.dart';
import 'settings/local_preferences.dart';

DesktopLifecycle? _desktopLifecycle;
DeepLinkRouter? _deepLinkRouter;
AndroidNotificationRouter? _androidNotificationRouter;

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  if (isTorcaAndroid) {
    // Keep Android's navigation controls transient. A swipe from an edge
    // reveals them temporarily, so system navigation remains accessible.
    SystemChrome.setEnabledSystemUIMode(SystemUiMode.immersiveSticky);
    SystemChrome.setSystemUIChangeCallback((systemOverlaysAreVisible) async {
      if (!systemOverlaysAreVisible) return;
      await Future<void>.delayed(const Duration(seconds: 2));
      await SystemChrome.setEnabledSystemUIMode(SystemUiMode.immersiveSticky);
    });
  }
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
  bool _initializing = false;
  RuntimeLifecycleObserver? _runtimeLifecycleObserver;

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
      await preferences.load();
      final EngineGateway gateway = await _openNativeGateway();
      _gateway = gateway;
      if (gateway is FfiEngineGateway) {
        preferences.syncNotificationsEnabled(
          gateway.snapshots.value.notificationsEnabled,
        );
        preferences.attachRuntimeNotificationSetting((enabled) async {
          await gateway.execute(SetNotificationsCommandDto(enabled: enabled));
        });
      }
      final navigation = AppNavigationController();
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
      final application = TorcaApp(
        gateway: gateway,
        navigation: navigation,
        preferences: preferences,
        onRetryBootstrap: gateway is StartupFailureGateway
            ? _retryStartup
            : null,
      );
      if (mounted) setState(() => _application = application);
    } finally {
      _initializing = false;
    }
  }

  Future<void> _retryStartup() async {
    if (_initializing || !mounted) return;
    final failedGateway = _gateway;
    _gateway = null;
    setState(() => _application = null);
    await failedGateway?.dispose();
    await _initialize();
  }

  @override
  void dispose() {
    _runtimeLifecycleObserver?.detach();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => _application ?? const _StartupScreen();
}

class _StartupScreen extends StatelessWidget {
  const _StartupScreen();

  @override
  Widget build(BuildContext context) => MaterialApp(
    debugShowCheckedModeBanner: false,
    theme: ThemeData.dark(useMaterial3: true),
    home: const Scaffold(
      body: SafeArea(
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                'Torca',
                style: TextStyle(fontSize: 32, fontWeight: FontWeight.w600),
              ),
              SizedBox(height: 24),
              CircularProgressIndicator(),
              SizedBox(height: 16),
              Text('Starting secure network…'),
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
