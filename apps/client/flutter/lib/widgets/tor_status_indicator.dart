import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';
import '../theme/app_semantic_colors.dart';
import 'connection_state_presenter.dart';

/// Provider-neutral commissioning indicator.
///
/// The old `TorStatusIndicator` name is kept below as a source-compatible
/// alias for callers that have not migrated yet. New screens must pass the
/// selected provider instead of assuming onion/Tor semantics.
class CommunicationStatusIndicator extends StatelessWidget {
  const CommunicationStatusIndicator({
    required this.state,
    required this.provider,
    this.onPressed,
    super.key,
  });

  final String state;
  final String provider;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final presentation = ConnectionStatePresenter.provider(
      state: state,
      provider: provider,
      icons: context.torcaIcons,
      strings: context.strings,
    );
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
              borderRadius: BorderRadius.circular(
                context.torcaTokens.radiusLarge,
              ),
              onTap: onPressed,
              child: chip,
            ),
    );
  }
}

/// @deprecated Use [CommunicationStatusIndicator] and provide the selected
/// provider explicitly.
@Deprecated('Use CommunicationStatusIndicator')
class TorStatusIndicator extends CommunicationStatusIndicator {
  const TorStatusIndicator({required super.state, super.onPressed, super.key})
    : super(provider: 'tor');
}
