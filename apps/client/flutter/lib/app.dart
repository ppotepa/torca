import 'package:flutter/material.dart';

import 'gateway/engine_gateway.dart';
import 'generated/torca_contract.dart';
import 'navigation/app_navigation_controller.dart';
import 'screens/conversation_screen.dart';
import 'screens/deep_link_join_screen.dart';
import 'screens/home_screen.dart';

class TorcaApp extends StatefulWidget {
  const TorcaApp({required this.gateway, required this.navigation, super.key});

  final EngineGateway gateway;
  final AppNavigationController navigation;

  @override
  State<TorcaApp> createState() => _TorcaAppState();
}

class _TorcaAppState extends State<TorcaApp> {
  final GlobalKey<NavigatorState> _navigatorKey = GlobalKey<NavigatorState>();

  @override
  void initState() {
    super.initState();
    widget.navigation.conversationRequest.addListener(_conversationRequested);
    widget.navigation.pairingCodeRequest.addListener(_pairingRequested);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _conversationRequested();
      _pairingRequested();
    });
  }

  @override
  void didUpdateWidget(covariant TorcaApp oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.navigation == widget.navigation) return;
    oldWidget.navigation.conversationRequest.removeListener(_conversationRequested);
    oldWidget.navigation.pairingCodeRequest.removeListener(_pairingRequested);
    widget.navigation.conversationRequest.addListener(_conversationRequested);
    widget.navigation.pairingCodeRequest.addListener(_pairingRequested);
  }

  @override
  void dispose() {
    widget.navigation.conversationRequest.removeListener(_conversationRequested);
    widget.navigation.pairingCodeRequest.removeListener(_pairingRequested);
    super.dispose();
  }

  void _conversationRequested() {
    final id = widget.navigation.conversationRequest.value;
    if (id == null) return;
    final navigator = _navigatorKey.currentState;
    if (navigator == null) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _conversationRequested());
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
    navigator.push<void>(MaterialPageRoute(
      builder: (_) => ConversationScreen(gateway: widget.gateway, conversation: conversation!),
    ));
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
    navigator.push<void>(MaterialPageRoute(
      builder: (_) => DeepLinkJoinScreen(gateway: widget.gateway, code: code),
    ));
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
        navigatorKey: _navigatorKey,
        title: 'Torca',
        debugShowCheckedModeBanner: false,
        theme: ThemeData(
          colorSchemeSeed: Colors.blueGrey,
          useMaterial3: true,
        ),
        home: HomeScreen(gateway: widget.gateway),
      );
}
