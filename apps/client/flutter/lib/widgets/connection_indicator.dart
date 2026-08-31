import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

import '../theme/app_semantic_colors.dart';
import 'connection_state_presenter.dart';

class ConnectionIndicator extends StatelessWidget {
  const ConnectionIndicator({
    required this.state,
    required this.blocked,
    this.provider = 'iroh',
    this.showLabel = true,
    super.key,
  });

  final String state;
  final bool blocked;
  final String provider;
  final bool showLabel;

  @override
  Widget build(BuildContext context) {
    final presentation = ConnectionStatePresenter.peer(
      state: state,
      blocked: blocked,
      icons: context.torcaIcons,
      provider: provider,
      strings: context.l10n,
    );
    final semantic = Theme.of(context).extension<AppSemanticColors>();
    final scheme = Theme.of(context).colorScheme;
    final color = switch (presentation.tone) {
      ConnectionTone.ready => semantic?.connectionReady ?? scheme.primary,
      ConnectionTone.connecting =>
        semantic?.connectionConnecting ?? scheme.tertiary,
      ConnectionTone.offline => semantic?.connectionOffline ?? scheme.outline,
      ConnectionTone.blocked => semantic?.destructive ?? scheme.error,
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
