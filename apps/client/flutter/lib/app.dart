import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

import 'gateway/engine_gateway.dart';
import 'generated/torca_contract.dart';
import 'localization/app_locale_mode.dart';
import 'localization/torca_strings.dart';
import 'navigation/app_navigation_controller.dart';
import 'screens/conversation_screen.dart';
import 'screens/deep_link_join_screen.dart';
import 'screens/diagnostics_screen.dart';
import 'screens/home_screen.dart';
import 'screens/pairing_screen.dart';
import 'screens/settings_screen.dart';
import 'settings/local_preferences.dart';
import 'settings/preferences_scope.dart';
import 'theme/app_theme.dart';

class TorcaApp extends StatefulWidget {
  const TorcaApp({
    required this.gateway,
    required this.navigation,
    required this.preferences,
    this.onRetryBootstrap,
    super.key,
  });

  final EngineGateway gateway;
  final AppNavigationController navigation;
  final LocalPreferences preferences;
  final VoidCallback? onRetryBootstrap;

  @override
  State<TorcaApp> createState() => _TorcaAppState();
}

class _TorcaAppState extends State<TorcaApp> {
  final GlobalKey<NavigatorState> _navigatorKey = GlobalKey<NavigatorState>();
  int _handledPairingRequest = 0;

  @override
  void initState() {
    super.initState();
    widget.navigation.conversationRequest.addListener(_conversationRequested);
    widget.navigation.pairingCodeRequest.addListener(_pairingRequested);
    widget.navigation.newPairingRequest.addListener(_newPairingRequested);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _conversationRequested();
      _pairingRequested();
      _newPairingRequested();
    });
  }

  @override
  void didUpdateWidget(covariant TorcaApp oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.navigation == widget.navigation) return;
    oldWidget.navigation.conversationRequest.removeListener(
      _conversationRequested,
    );
    oldWidget.navigation.pairingCodeRequest.removeListener(_pairingRequested);
    oldWidget.navigation.newPairingRequest.removeListener(_newPairingRequested);
    widget.navigation.conversationRequest.addListener(_conversationRequested);
    widget.navigation.pairingCodeRequest.addListener(_pairingRequested);
    widget.navigation.newPairingRequest.addListener(_newPairingRequested);
    _handledPairingRequest = widget.navigation.newPairingRequest.value;
  }

  @override
  void dispose() {
    widget.navigation.conversationRequest.removeListener(
      _conversationRequested,
    );
    widget.navigation.pairingCodeRequest.removeListener(_pairingRequested);
    widget.navigation.newPairingRequest.removeListener(_newPairingRequested);
    super.dispose();
  }

  void _conversationRequested() {
    final id = widget.navigation.conversationRequest.value;
    if (id == null) return;
    final navigator = _navigatorKey.currentState;
    if (navigator == null) {
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => _conversationRequested(),
      );
      return;
    }
    widget.navigation.clearConversationRequest();
    ConversationDto? conversation;
    for (final candidate in widget.gateway.snapshots.value.conversations) {
      if (candidate.id == id) {
        conversation = candidate;
        break;
      }
    }
    if (conversation == null) return;
    navigator.push<void>(
      MaterialPageRoute(
        builder: (_) => ConversationScreen(
          gateway: widget.gateway,
          conversation: conversation!,
        ),
      ),
    );
  }

  void _pairingRequested() {
    final code = widget.navigation.pairingCodeRequest.value;
    if (code == null) return;
    final navigator = _navigatorKey.currentState;
    if (navigator == null) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _pairingRequested());
      return;
    }
    widget.navigation.clearPairingRequest();
    navigator.push<void>(
      MaterialPageRoute(
        builder: (_) => DeepLinkJoinScreen(gateway: widget.gateway, code: code),
      ),
    );
  }

  void _newPairingRequested() {
    final request = widget.navigation.newPairingRequest.value;
    if (request == _handledPairingRequest) return;
    final navigator = _navigatorKey.currentState;
    if (navigator == null) {
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => _newPairingRequested(),
      );
      return;
    }
    _handledPairingRequest = request;
    _openPairing();
  }

  void _openPairing() {
    _navigatorKey.currentState?.push<void>(
      MaterialPageRoute(builder: (_) => PairingScreen(gateway: widget.gateway)),
    );
  }

  void _openSettings() {
    _navigatorKey.currentState?.push<void>(
      MaterialPageRoute(
        builder: (_) => SettingsScreen(preferences: widget.preferences),
      ),
    );
  }

  void _openDiagnostics() {
    _navigatorKey.currentState?.push<void>(
      MaterialPageRoute(
        builder: (_) => DiagnosticsScreen(gateway: widget.gateway),
      ),
    );
  }

  void _dismissTopRoute() {
    final navigator = _navigatorKey.currentState;
    if (navigator != null && navigator.canPop()) navigator.pop();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: widget.preferences,
    builder: (context, _) => MaterialApp(
      navigatorKey: _navigatorKey,
      title: 'Torca',
      debugShowCheckedModeBanner: false,
      locale: widget.preferences.localeMode.locale,
      supportedLocales: TorcaStrings.supportedLocales,
      localizationsDelegates: const <LocalizationsDelegate<dynamic>>[
        TorcaStrings.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      themeMode: AppTheme.materialMode(widget.preferences.themeMode),
      builder: (context, child) => PreferencesScope(
        preferences: widget.preferences,
        child: Shortcuts(
          shortcuts: const <ShortcutActivator, Intent>{
            SingleActivator(LogicalKeyboardKey.keyN, control: true):
                _NewPairingIntent(),
            SingleActivator(LogicalKeyboardKey.comma, control: true):
                _SettingsIntent(),
            SingleActivator(
              LogicalKeyboardKey.keyD,
              control: true,
              shift: true,
            ): _DiagnosticsIntent(),
            SingleActivator(LogicalKeyboardKey.escape): _DismissIntent(),
          },
          child: Actions(
            actions: <Type, Action<Intent>>{
              _NewPairingIntent: CallbackAction<_NewPairingIntent>(
                onInvoke: (_) {
                  _openPairing();
                  return null;
                },
              ),
              _SettingsIntent: CallbackAction<_SettingsIntent>(
                onInvoke: (_) {
                  _openSettings();
                  return null;
                },
              ),
              _DiagnosticsIntent: CallbackAction<_DiagnosticsIntent>(
                onInvoke: (_) {
                  _openDiagnostics();
                  return null;
                },
              ),
              _DismissIntent: CallbackAction<_DismissIntent>(
                onInvoke: (_) {
                  _dismissTopRoute();
                  return null;
                },
              ),
            },
            child: FocusTraversalGroup(
              policy: ReadingOrderTraversalPolicy(),
              child: child ?? const SizedBox.shrink(),
            ),
          ),
        ),
      ),
      home: HomeScreen(
        gateway: widget.gateway,
        preferences: widget.preferences,
        onRetryBootstrap: widget.onRetryBootstrap,
      ),
    ),
  );
}

class _NewPairingIntent extends Intent {
  const _NewPairingIntent();
}

class _SettingsIntent extends Intent {
  const _SettingsIntent();
}

class _DiagnosticsIntent extends Intent {
  const _DiagnosticsIntent();
}

class _DismissIntent extends Intent {
  const _DismissIntent();
}
