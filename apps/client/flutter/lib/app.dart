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
  final Set<String> _knownContactIds = <String>{};
  bool _contactBaselineCaptured = false;
  bool _pairingPromptOpen = false;
  String? _scheduledPairingPromptId;

  @override
  void initState() {
    super.initState();
    widget.navigation.conversationRequest.addListener(_conversationRequested);
    widget.navigation.pairingCodeRequest.addListener(_pairingRequested);
    widget.navigation.newPairingRequest.addListener(_newPairingRequested);
    widget.navigation.pairingSessionRequest.addListener(
      _pairingSessionRequested,
    );
    widget.gateway.snapshots.addListener(_pairingSnapshotChanged);
    PairingModalRegistry.instance.addListener(_pairingSnapshotChanged);
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
    if (oldWidget.navigation != widget.navigation) {
      oldWidget.navigation.conversationRequest.removeListener(
        _conversationRequested,
      );
      oldWidget.navigation.pairingCodeRequest.removeListener(_pairingRequested);
      oldWidget.navigation.newPairingRequest.removeListener(
        _newPairingRequested,
      );
      oldWidget.navigation.pairingSessionRequest.removeListener(
        _pairingSessionRequested,
      );
      widget.navigation.conversationRequest.addListener(_conversationRequested);
      widget.navigation.pairingCodeRequest.addListener(_pairingRequested);
      widget.navigation.newPairingRequest.addListener(_newPairingRequested);
      widget.navigation.pairingSessionRequest.addListener(
        _pairingSessionRequested,
      );
      _handledPairingRequest = widget.navigation.newPairingRequest.value;
    }
    if (oldWidget.gateway != widget.gateway) {
      oldWidget.gateway.snapshots.removeListener(_pairingSnapshotChanged);
      widget.gateway.snapshots.addListener(_pairingSnapshotChanged);
      _knownContactIds.clear();
      _pairingPromptsShown.clear();
      _contactBaselineCaptured = false;
      _scheduledPairingPromptId = null;
    }
  }

  @override
  void dispose() {
    widget.navigation.conversationRequest.removeListener(
      _conversationRequested,
    );
    widget.navigation.pairingCodeRequest.removeListener(_pairingRequested);
    widget.navigation.newPairingRequest.removeListener(_newPairingRequested);
    widget.navigation.pairingSessionRequest.removeListener(
      _pairingSessionRequested,
    );
    widget.gateway.snapshots.removeListener(_pairingSnapshotChanged);
    PairingModalRegistry.instance.removeListener(_pairingSnapshotChanged);
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
          preferences: widget.preferences,
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

  void _pairingSessionRequested() {
    final id = widget.navigation.pairingSessionRequest.value;
    if (id == null) return;
    final navigator = _navigatorKey.currentState;
    if (navigator == null) {
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => _pairingSessionRequested(),
      );
      return;
    }
    widget.navigation.clearPairingSessionRequest();
    PairingDto? pairing;
    for (final candidate in widget.gateway.snapshots.value.pairings) {
      if (candidate.id == id) {
        pairing = candidate;
        break;
      }
    }
    if (pairing != null) {
      unawaited(
        showPairingSessionModal(navigator.context, widget.gateway, pairing),
      );
    }
  }

  bool _needsPairingDecision(PairingDto pairing) =>
      pairing.typedRole == PairingRole.creator &&
      (pairing.typedState == PairingState.peerJoined ||
          pairing.typedState == PairingState.awaitingApproval);

  void _pairingSnapshotChanged() {
    final snapshot = widget.gateway.snapshots.value;
    final pairings = snapshot.pairings;
    if (_contactBaselineCaptured) {
      for (final contact in snapshot.contacts) {
        if (!_knownContactIds.contains(contact.id)) {
          final context = _navigatorKey.currentContext;
          if (mounted && context != null) {
            ScaffoldMessenger.of(context)
              ..hideCurrentSnackBar()
              ..showSnackBar(
                SnackBar(
                  content: Text(
                    context.strings.contactAddedToContacts(contact.displayName),
                  ),
                ),
              );
          }
        }
      }
    }
    _knownContactIds
      ..clear()
      ..addAll(snapshot.contacts.map((contact) => contact.id));
    _contactBaselineCaptured = true;
    if (!mounted || _pairingPromptOpen || _scheduledPairingPromptId != null) {
      return;
    }
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
    _scheduledPairingPromptId = candidate.id;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _showIncomingPairing(candidate!);
    });
    // Registry ownership can be released outside a frame (for example while
    // closing a native/deep-link surface). A post-frame callback alone does
    // not request a frame, so explicitly wake the scheduler.
    WidgetsBinding.instance.scheduleFrame();
  }

  Future<void> _showIncomingPairing(PairingDto pairing) async {
    final navigator = _navigatorKey.currentState;
    if (navigator == null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _showIncomingPairing(pairing);
      });
      return;
    }
    // Claim before pushing the route.  Snapshot updates can arrive between
    // candidate selection and showDialog; without this reservation another
    // entry point could open a second surface for the same pairing session.
    final modalRegistry = PairingModalRegistry.instance;
    if (modalRegistry.owns(pairing.id)) {
      _scheduledPairingPromptId = null;
      return;
    }
    _pairingPromptOpen = true;
    _scheduledPairingPromptId = null;
    modalRegistry.claim(pairing.id);
    _pairingPromptsShown.add(pairing.id);
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
      modalRegistry.release(pairing.id);
      _pairingPromptOpen = false;
      _pairingSnapshotChanged();
    }
  }

  void _openPairing() {
    // Pairing has two explicit product flows: joining belongs to the global
    // add-contact action, while creating belongs exclusively to Invitations.
    // Keep the platform shortcut aligned with the same focused join modal used
    // by Contacts; invitation creation remains in the Invitations section.
    final context = _navigatorKey.currentContext;
    if (context != null) {
      unawaited(showJoinInvitationModal(context, widget.gateway));
    }
  }

  void _openSettings() {
    _navigatorKey.currentState?.push<void>(
      MaterialPageRoute(
        builder: (_) => SettingsScreen(
          preferences: widget.preferences,
          gateway: widget.gateway,
        ),
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
    listenable: widget.preferences.shellChanges,
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
