import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

import 'gateway/engine_gateway.dart';
import 'generated/torca_contract.dart';
import 'localization/app_locale_mode.dart';
import 'localization/torca_strings.dart';
import 'navigation/app_navigation_controller.dart';
import 'screens/conversation_screen.dart';
import 'screens/diagnostics_screen.dart';
import 'screens/home_screen.dart';
import 'screens/pairing_screen.dart';
import 'screens/settings_screen.dart';
import 'settings/local_preferences.dart';
import 'settings/preferences_scope.dart';
import 'theme/app_theme.dart';
import 'widgets/incoming_pairing_dialog.dart';
import 'widgets/pairing_modal_registry.dart';
import 'widgets/runtime_network_status.dart';

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
  final Set<String> _pairingPromptsShown = <String>{};
  final Map<String, PairingState> _pairingStates = <String, PairingState>{};
  bool _pairingBaselineCaptured = false;
  bool _pairingPromptOpen = false;

  @override
  void initState() {
    super.initState();
    widget.navigation.conversationRequest.addListener(_conversationRequested);
    widget.navigation.pairingCodeRequest.addListener(_pairingRequested);
    widget.navigation.newPairingRequest.addListener(_newPairingRequested);
    widget.gateway.snapshots.addListener(_pairingSnapshotChanged);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _conversationRequested();
      _pairingRequested();
      _newPairingRequested();
      _pairingSnapshotChanged();
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
    oldWidget.gateway.snapshots.removeListener(_pairingSnapshotChanged);
    widget.gateway.snapshots.addListener(_pairingSnapshotChanged);
    _handledPairingRequest = widget.navigation.newPairingRequest.value;
    _pairingStates.clear();
    _pairingBaselineCaptured = false;
  }

  @override
  void dispose() {
    widget.navigation.conversationRequest.removeListener(
      _conversationRequested,
    );
    widget.navigation.pairingCodeRequest.removeListener(_pairingRequested);
    widget.navigation.newPairingRequest.removeListener(_newPairingRequested);
    widget.gateway.snapshots.removeListener(_pairingSnapshotChanged);
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
    // Deep links must use the exact same join composer as Contacts. The old
    // route rendered a second, legacy screen on Android and was the source of
    // the keyboard/input and layout divergence between platforms.
    unawaited(
      showJoinInvitationModal(
        navigator.context,
        widget.gateway,
        initialCode: code,
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

  bool _needsPairingDecision(PairingDto pairing) =>
      pairing.typedRole == PairingRole.creator &&
      (pairing.typedState == PairingState.peerJoined ||
          pairing.typedState == PairingState.awaitingApproval);

  void _pairingSnapshotChanged() {
    final pairings = widget.gateway.snapshots.value.pairings;
    if (_pairingBaselineCaptured) {
      for (final pairing in pairings) {
        final previous = _pairingStates[pairing.id];
        if (previous != null &&
            previous != PairingState.completed &&
            pairing.typedState == PairingState.completed) {
          final name = pairing.remoteDisplayName?.trim();
          final label = name == null || name.isEmpty ? 'Contact' : name;
          final context = _navigatorKey.currentContext;
          if (mounted && context != null) {
            ScaffoldMessenger.of(context)
              ..hideCurrentSnackBar()
              ..showSnackBar(
                SnackBar(content: Text('$label accepted your invitation')),
              );
          }
        }
      }
    }
    _pairingStates
      ..clear()
      ..addEntries(
        pairings.map((pairing) => MapEntry(pairing.id, pairing.typedState)),
      );
    _pairingBaselineCaptured = true;
    if (!mounted || _pairingPromptOpen) return;
    PairingDto? candidate;
    for (final pairing in pairings) {
      if (_needsPairingDecision(pairing) &&
          !PairingModalRegistry.instance.owns(pairing.id) &&
          !_pairingPromptsShown.contains(pairing.id)) {
        candidate = pairing;
        break;
      }
    }
    if (candidate == null) return;
    _pairingPromptsShown.add(candidate.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _showIncomingPairing(candidate!);
    });
  }

  Future<void> _showIncomingPairing(PairingDto pairing) async {
    final navigator = _navigatorKey.currentState;
    if (navigator == null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _showIncomingPairing(pairing);
      });
      return;
    }
    _pairingPromptOpen = true;
    try {
      await showDialog<void>(
        context: navigator.context,
        // The prompt is an attention surface, not a modal lock. Users must
        // be able to dismiss it with a tap outside or the system back action;
        // the pairing remains in the Invitations list until explicitly
        // accepted, rejected or cancelled.
        barrierDismissible: true,
        builder: (_) =>
            IncomingPairingDialog(gateway: widget.gateway, pairing: pairing),
      );
    } finally {
      _pairingPromptOpen = false;
      _pairingSnapshotChanged();
    }
  }

  void _openPairing() {
    // Pairing has two explicit product flows: joining belongs to the global
    // add-contact action, while creating belongs exclusively to Invitations.
    // Do not route through the legacy combined PairingScreen; it reintroduces
    // the old Create/Join UI on Android and desktop shortcuts.
    final context = _navigatorKey.currentContext;
    if (context != null) {
      unawaited(showJoinInvitationModal(context, widget.gateway));
    }
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
      theme: AppTheme.light(widget.preferences.appearance),
      darkTheme: AppTheme.dark(widget.preferences.appearance),
      themeMode: AppTheme.materialMode(widget.preferences.themeMode),
      themeAnimationDuration: Duration.zero,
      builder: (context, child) => RuntimeStatusScope(
        gateway: widget.gateway,
        child: PreferencesScope(
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
