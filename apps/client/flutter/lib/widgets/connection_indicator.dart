import 'package:flutter/material.dart';

import '../theme/app_semantic_colors.dart';
import 'connection_state_presenter.dart';

class ConnectionIndicator extends StatelessWidget {
  const ConnectionIndicator({
    required this.state,
    required this.blocked,
    this.showLabel = true,
    super.key,
  });

  final String state;
  final bool blocked;
  final bool showLabel;

  @override
  Widget build(BuildContext context) {
    final presentation = ConnectionStatePresenter.peer(
      state: state,
      blocked: blocked,
    );
    final color = switch (presentation.tone) {
      ConnectionTone.ready => context.semanticColors.connectionReady,
      ConnectionTone.connecting => context.semanticColors.connectionConnecting,
      ConnectionTone.offline => context.semanticColors.connectionOffline,
      ConnectionTone.blocked => context.semanticColors.destructive,
    };
    return Tooltip(
      message: presentation.tooltip,
      child: Semantics(
        label: presentation.label,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(presentation.icon, size: 18, color: color),
            if (showLabel) ...<Widget>[
              const SizedBox(width: 5),
              Text(presentation.shortLabel),
            ],
          ],
        ),
      ),
    );
  }
}
