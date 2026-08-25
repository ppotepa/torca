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
    String provider = 'tor',
    TorcaStrings? strings,
  }) {
    final labels = strings ?? const TorcaStrings(Locale('en'));
    final providerLabel = provider.isEmpty ? 'tor' : provider;
    final directLabel = providerLabel == 'tor'
        ? labels.directP2pOverTor
        : labels.directProviderContact(providerLabel);
    final directTooltip = providerLabel == 'tor'
        ? labels.directP2pOverTor
        : labels.directProviderContact(providerLabel);
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
        label: directLabel,
        shortLabel: labels.p2pShort,
        tooltip: directTooltip,
        icon: icons.online,
        tone: ConnectionTone.ready,
      ),
      'connecting' || 'handshaking' => ConnectionPresentation(
        label: labels.connecting,
        shortLabel: labels.connecting,
        tooltip: providerLabel == 'tor'
            ? labels.connectingPeerThroughTor
            : labels.connectingPeerThrough(providerLabel),
        icon: icons.reconnect,
        tone: ConnectionTone.connecting,
      ),
      'reconnecting' => ConnectionPresentation(
        label: labels.reconnecting,
        shortLabel: labels.reconnecting,
        tooltip: providerLabel == 'tor'
            ? labels.reconnectingPeerThroughTor
            : labels.reconnectingPeerThrough(providerLabel),
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
    return provider(
      state: state,
      provider: 'tor',
      icons: icons,
      strings: labels,
    );
  }

  static ConnectionPresentation provider({
    required String state,
    required String provider,
    required TorcaIconSet icons,
    TorcaStrings? strings,
  }) {
    final labels = strings ?? const TorcaStrings(Locale('en'));
    return switch (state) {
      'ready' => ConnectionPresentation(
        label: labels.providerReady(provider),
        shortLabel: labels.providerName(provider),
        tooltip: labels.providerReady(provider),
        icon: icons.identity,
        tone: ConnectionTone.ready,
      ),
      'starting' || 'connecting' => ConnectionPresentation(
        label: labels.providerStarting(provider),
        shortLabel: labels.startingShort,
        tooltip: labels.providerStarting(provider),
        icon: icons.identity,
        tone: ConnectionTone.connecting,
      ),
      'reconnecting' => ConnectionPresentation(
        label: labels.providerReconnecting(provider),
        shortLabel: labels.reconnectingShort,
        tooltip: labels.providerReconnecting(provider),
        icon: icons.reconnect,
        tone: ConnectionTone.connecting,
      ),
      _ => ConnectionPresentation(
        label: labels.providerStateLabel(provider, state),
        shortLabel: state.isEmpty ? labels.offlineShort : state,
        tooltip: labels.providerStateLabel(provider, state),
        icon: icons.identity,
        tone: ConnectionTone.offline,
      ),
    };
  }
}
