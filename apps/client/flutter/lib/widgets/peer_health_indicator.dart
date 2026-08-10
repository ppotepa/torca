import 'package:flutter/material.dart';

import '../generated/torca_contract.dart';
import '../theme/app_semantic_colors.dart';

class PeerHealthIndicator extends StatefulWidget {
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
  State<PeerHealthIndicator> createState() => _PeerHealthIndicatorState();
}

class _PeerHealthIndicatorState extends State<PeerHealthIndicator>
    with SingleTickerProviderStateMixin {
  late final AnimationController _pulse = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 680),
  );

  @override
  void didUpdateWidget(covariant PeerHealthIndicator oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.health.activitySequence != widget.health.activitySequence &&
        widget.health.activitySequence > 0) {
      _pulse.forward(from: 0);
    }
  }

  @override
  void dispose() {
    _pulse.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final health = widget.health;
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
    final child = AnimatedBuilder(
      animation: _pulse,
      builder: (context, _) {
        final glow = _pulse.value < 0.5
            ? _pulse.value * 2
            : (1 - _pulse.value) * 2;
        return Semantics(
          label: 'Connection quality $quality$rtt',
          button: widget.onPressed != null,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Container(
                padding: EdgeInsets.all(glow * 2),
                decoration: BoxDecoration(
                  color: color.withValues(alpha: glow * 0.22),
                  shape: BoxShape.circle,
                ),
                child: Icon(icon, size: 17, color: color),
              ),
              if (widget.showLabel) ...<Widget>[
                const SizedBox(width: 5),
                Text(quality),
              ],
            ],
          ),
        );
      },
    );
    return Tooltip(
      message: 'Connection quality: $quality$rtt',
      child: widget.onPressed == null
          ? child
          : InkWell(
              borderRadius: BorderRadius.circular(16),
              onTap: widget.onPressed,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 3),
                child: child,
              ),
            ),
    );
  }

  String _qualityLabel(String quality) => switch (quality) {
    'excellent' => 'Excellent',
    'good' => 'Good',
    'fair' => 'Fair',
    'poor' => 'Poor',
    _ => 'Unknown',
  };
}
