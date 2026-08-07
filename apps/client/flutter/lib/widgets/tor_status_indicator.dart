import 'package:flutter/material.dart';

import '../theme/app_semantic_colors.dart';
import 'connection_state_presenter.dart';

class TorStatusIndicator extends StatelessWidget {
  const TorStatusIndicator({required this.state, this.onPressed, super.key});

  final String state;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final presentation = ConnectionStatePresenter.tor(state);
    final color = switch (presentation.tone) {
      ConnectionTone.ready => context.semanticColors.connectionReady,
      ConnectionTone.connecting => context.semanticColors.connectionConnecting,
      ConnectionTone.offline => context.semanticColors.connectionOffline,
      ConnectionTone.blocked => context.semanticColors.destructive,
    };
    final chip = Chip(
      avatar: Icon(presentation.icon, size: 17, color: color),
      label: Text(presentation.shortLabel),
      visualDensity: VisualDensity.compact,
    );
    return Tooltip(
      message: presentation.tooltip,
      child: onPressed == null
          ? chip
          : InkWell(
              borderRadius: BorderRadius.circular(18),
              onTap: onPressed,
              child: chip,
            ),
    );
  }
}
