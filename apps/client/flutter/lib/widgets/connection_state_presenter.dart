import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

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
  }) {
    if (blocked) {
      return ConnectionPresentation(
        label: 'Blocked',
        shortLabel: 'Blocked',
        tooltip: 'Contact is blocked',
        icon: icons.block,
        tone: ConnectionTone.blocked,
      );
    }
    return switch (state) {
      'ready' => ConnectionPresentation(
        label: 'Direct P2P over Tor',
        shortLabel: 'P2P',
        tooltip: 'Direct P2P over Tor',
        icon: icons.online,
        tone: ConnectionTone.ready,
      ),
      'connecting' || 'handshaking' => ConnectionPresentation(
        label: 'Connecting',
        shortLabel: '…',
        tooltip: 'Connecting to peer through Tor',
        icon: icons.reconnect,
        tone: ConnectionTone.connecting,
      ),
      'reconnecting' => ConnectionPresentation(
        label: 'Reconnecting',
        shortLabel: '…',
        tooltip: 'Reconnecting to peer through Tor',
        icon: icons.reconnect,
        tone: ConnectionTone.connecting,
      ),
      _ => ConnectionPresentation(
        label: 'Offline',
        shortLabel: 'offline',
        tooltip: 'Peer is offline',
        icon: icons.error,
        tone: ConnectionTone.offline,
      ),
    };
  }

  static ConnectionPresentation tor(String state, TorcaIconSet icons) =>
      switch (state) {
        'ready' => ConnectionPresentation(
          label: 'Tor ready',
          shortLabel: 'Tor',
          tooltip: 'Tor is ready',
          icon: icons.identity,
          tone: ConnectionTone.ready,
        ),
        'starting' || 'connecting' => ConnectionPresentation(
          label: 'Tor starting',
          shortLabel: 'Starting',
          tooltip: 'Tor is starting',
          icon: icons.identity,
          tone: ConnectionTone.connecting,
        ),
        'reconnecting' => ConnectionPresentation(
          label: 'Tor reconnecting',
          shortLabel: 'Reconnecting',
          tooltip: 'Tor is reconnecting',
          icon: icons.reconnect,
          tone: ConnectionTone.connecting,
        ),
        _ => ConnectionPresentation(
          label: 'Tor ${state.isEmpty ? 'offline' : state}',
          shortLabel: state.isEmpty ? 'Offline' : state,
          tooltip: 'Tor: ${state.isEmpty ? 'offline' : state}',
          icon: icons.identity,
          tone: ConnectionTone.offline,
        ),
      };
}
