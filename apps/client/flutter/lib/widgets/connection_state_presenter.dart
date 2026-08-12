import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';

enum ConnectionTone { ready, connecting, offline, blocked }

@immutable
class ConnectionPresentation {
  const ConnectionPresentation({
    required this.label,
    required this.shortLabel,
    required this.tooltip,
    required this.icon,
    required this.tone,
  });

  final String label;
  final String shortLabel;
  final String tooltip;
  final IconData icon;
  final ConnectionTone tone;
}

abstract final class ConnectionStatePresenter {
  static ConnectionPresentation peer({
    required String state,
    required bool blocked,
    required TorcaIconSet icons,
    TorcaStrings? strings,
  }) {
    final labels = strings ?? const TorcaStrings(Locale('en'));
    if (blocked) {
      return ConnectionPresentation(
        label: labels.blocked,
        shortLabel: labels.blocked,
        tooltip: labels.contactBlocked,
        icon: icons.block,
        tone: ConnectionTone.blocked,
      );
    }
    return switch (state) {
      'ready' => ConnectionPresentation(
        label: labels.directP2pOverTor,
        shortLabel: labels.p2pShort,
        tooltip: labels.directP2pOverTor,
        icon: icons.online,
        tone: ConnectionTone.ready,
      ),
      'connecting' || 'handshaking' => ConnectionPresentation(
        label: labels.connecting,
        shortLabel: labels.connecting,
        tooltip: labels.connectingPeerThroughTor,
        icon: icons.reconnect,
        tone: ConnectionTone.connecting,
      ),
      'reconnecting' => ConnectionPresentation(
        label: labels.reconnecting,
        shortLabel: labels.reconnecting,
        tooltip: labels.reconnectingPeerThroughTor,
        icon: icons.reconnect,
        tone: ConnectionTone.connecting,
      ),
      _ => ConnectionPresentation(
        label: labels.peerOffline,
        shortLabel: labels.offlineShort,
        tooltip: labels.peerOffline,
        icon: icons.error,
        tone: ConnectionTone.offline,
      ),
    };
  }

  static ConnectionPresentation tor(
    String state,
    TorcaIconSet icons, [
    TorcaStrings? strings,
  ]) {
    final labels = strings ?? const TorcaStrings(Locale('en'));
    return switch (state) {
      'ready' => ConnectionPresentation(
        label: labels.torReady,
        shortLabel: labels.torShort,
        tooltip: labels.torReady,
        icon: icons.identity,
        tone: ConnectionTone.ready,
      ),
      'starting' || 'connecting' => ConnectionPresentation(
        label: labels.torStarting,
        shortLabel: labels.startingShort,
        tooltip: labels.torStarting,
        icon: icons.identity,
        tone: ConnectionTone.connecting,
      ),
      'reconnecting' => ConnectionPresentation(
        label: labels.torReconnecting,
        shortLabel: labels.reconnectingShort,
        tooltip: labels.torReconnecting,
        icon: icons.reconnect,
        tone: ConnectionTone.connecting,
      ),
      _ => ConnectionPresentation(
        label: labels.torStateLabel(state),
        shortLabel: state.isEmpty ? labels.offlineShort : state,
        tooltip: labels.torStateLabel(state),
        icon: icons.identity,
        tone: ConnectionTone.offline,
      ),
    };
  }
}
