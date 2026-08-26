import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';
import '../theme/app_semantic_colors.dart';
import 'connection_state_presenter.dart';

class ConnectionIndicator extends StatelessWidget {
  const ConnectionIndicator({
    required this.state,
    required this.blocked,
    // Kept for widgets compiled against the legacy contract. Screens that
    // render a contact always pass its provider explicitly.
    this.provider = 'tor',
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
      strings: context.strings,
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
