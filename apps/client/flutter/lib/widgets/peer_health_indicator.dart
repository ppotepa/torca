import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

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
  bool _animationsEnabled = true;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _animationsEnabled =
        context.torcaTokens.animationDuration != Duration.zero &&
        !MediaQuery.disableAnimationsOf(context);
    if (!_animationsEnabled) {
      _pulse
        ..stop()
        ..value = 0;
    }
  }

  @override
  void didUpdateWidget(covariant PeerHealthIndicator oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_animationsEnabled &&
        oldWidget.health.activitySequence != widget.health.activitySequence &&
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
    final quality = _qualityLabel(health.typedQuality);
    final color = switch (health.typedQuality) {
      PeerHealthQuality.excellent ||
      PeerHealthQuality.good => context.semanticColors.connectionReady,
      PeerHealthQuality.fair => context.semanticColors.connectionConnecting,
      PeerHealthQuality.poor => context.semanticColors.warning,
      _ => context.semanticColors.connectionOffline,
    };
    final icon = switch (health.typedQuality) {
      PeerHealthQuality.excellent ||
      PeerHealthQuality.good => context.torcaIcons.online,
      PeerHealthQuality.fair ||
      PeerHealthQuality.poor => context.torcaIcons.warning,
      _ => context.torcaIcons.error,
    };
    final rtt = health.rttMs == null ? '' : ' · ${health.rttMs} ms';
    final child = AnimatedBuilder(
      animation: _pulse,
      builder: (context, _) {
        final glow = _pulse.value < 0.5
            ? _pulse.value * 2
            : (1 - _pulse.value) * 2;
        return Semantics(
          label: context.l10n.connectionQuality(quality, rtt),
          button: widget.onPressed != null,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Container(
                padding: EdgeInsets.all(glow * 2),
                decoration: BoxDecoration(
                  color: color.withValues(alpha: glow * 0.22),
                  borderRadius: BorderRadius.circular(
                    context.torcaTokens.radiusLarge,
                  ),
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
              borderRadius: BorderRadius.circular(
                context.torcaTokens.radiusLarge,
              ),
              onTap: widget.onPressed,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 3),
                child: child,
              ),
            ),
    );
  }

  String _qualityLabel(PeerHealthQuality quality) => switch (quality) {
    PeerHealthQuality.excellent => 'Excellent',
    PeerHealthQuality.good => 'Good',
    PeerHealthQuality.fair => 'Fair',
    PeerHealthQuality.poor => 'Poor',
    _ => 'Unknown',
  };
}
