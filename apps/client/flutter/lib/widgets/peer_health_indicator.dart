import 'package:flutter/material.dart';

import '../generated/torca_contract.dart';
import '../theme/app_semantic_colors.dart';

class PeerHealthIndicator extends StatelessWidget {
  const PeerHealthIndicator({
    required this.health,
    this.showLabel = true,
    this.onPressed,
    super.key,
  });

  final PeerHealthDto health;
  final bool showLabel;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final quality = _qualityLabel(health.quality);
    final color = switch (health.quality) {
      'excellent' || 'good' => context.semanticColors.connectionReady,
      'fair' => context.semanticColors.connectionConnecting,
      'poor' => context.semanticColors.warning,
      _ => context.semanticColors.connectionOffline,
    };
    final icon = switch (health.quality) {
      'excellent' => Icons.signal_cellular_alt,
      'good' => Icons.network_cell,
      'fair' => Icons.network_cell,
      'poor' => Icons.signal_cellular_connected_no_internet_4_bar,
      _ => Icons.signal_cellular_off,
    };
    final rtt = health.rttMs == null ? '' : ' · ${health.rttMs} ms';
    final child = Semantics(
      label: 'Connection quality $quality$rtt',
      button: onPressed != null,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Icon(icon, size: 17, color: color),
          if (showLabel) ...<Widget>[const SizedBox(width: 5), Text(quality)],
        ],
      ),
    );
    return Tooltip(
      message: 'Connection quality: $quality$rtt',
      child: onPressed == null
          ? child
          : InkWell(
              borderRadius: BorderRadius.circular(16),
              onTap: onPressed,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 3),
                child: child,
              ),
            ),
    );
  }

  static String _qualityLabel(String quality) => switch (quality) {
    'excellent' => 'Excellent',
    'good' => 'Good',
    'fair' => 'Fair',
    'poor' => 'Poor',
    _ => 'Unknown',
  };
}
