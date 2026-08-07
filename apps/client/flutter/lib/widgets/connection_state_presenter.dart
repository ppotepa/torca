import 'package:flutter/material.dart';

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
  }) {
    if (blocked) {
      return const ConnectionPresentation(
        label: 'Blocked',
        shortLabel: 'Blocked',
        tooltip: 'Contact is blocked',
        icon: Icons.block,
        tone: ConnectionTone.blocked,
      );
    }
    return switch (state) {
      'ready' => const ConnectionPresentation(
          label: 'Direct P2P over Tor',
          shortLabel: 'P2P',
          tooltip: 'Direct P2P over Tor',
          icon: Icons.hub,
          tone: ConnectionTone.ready,
        ),
      'connecting' || 'handshaking' => const ConnectionPresentation(
          label: 'Connecting',
          shortLabel: '…',
          tooltip: 'Connecting to peer through Tor',
          icon: Icons.sync,
          tone: ConnectionTone.connecting,
        ),
      'reconnecting' => const ConnectionPresentation(
          label: 'Reconnecting',
          shortLabel: '…',
          tooltip: 'Reconnecting to peer through Tor',
          icon: Icons.sync,
          tone: ConnectionTone.connecting,
        ),
      _ => const ConnectionPresentation(
          label: 'Offline',
          shortLabel: 'offline',
          tooltip: 'Peer is offline',
          icon: Icons.cloud_off_outlined,
          tone: ConnectionTone.offline,
        ),
    };
  }

  static ConnectionPresentation tor(String state) => switch (state) {
        'ready' => const ConnectionPresentation(
            label: 'Tor ready',
            shortLabel: 'Tor',
            tooltip: 'Tor is ready',
            icon: Icons.security,
            tone: ConnectionTone.ready,
          ),
        'starting' || 'connecting' => const ConnectionPresentation(
            label: 'Tor starting',
            shortLabel: 'Starting',
            tooltip: 'Tor is starting',
            icon: Icons.security_outlined,
            tone: ConnectionTone.connecting,
          ),
        'reconnecting' => const ConnectionPresentation(
            label: 'Tor reconnecting',
            shortLabel: 'Reconnecting',
            tooltip: 'Tor is reconnecting',
            icon: Icons.security_outlined,
            tone: ConnectionTone.connecting,
          ),
        _ => ConnectionPresentation(
            label: 'Tor ${state.isEmpty ? 'offline' : state}',
            shortLabel: state.isEmpty ? 'Offline' : state,
            tooltip: 'Tor: ${state.isEmpty ? 'offline' : state}',
            icon: Icons.security_outlined,
            tone: ConnectionTone.offline,
          ),
      };
}
