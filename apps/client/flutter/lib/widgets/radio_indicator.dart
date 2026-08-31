import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';

/// Compact, theme-aware Radio presence marker shared by contact and
/// conversation lists. It reflects runtime state only and never starts work.
class RadioIndicator extends StatelessWidget {
  const RadioIndicator({
    required this.radio,
    this.session,
    this.contactName,
    super.key,
  });

  final RadioContactDto? radio;
  final RadioSessionDto? session;
  final String? contactName;

  @override
  Widget build(BuildContext context) {
    final value = radio;
    if (value == null ||
        (!value.localEnabled &&
            value.typedRemoteState != RadioRemoteState.enabled)) {
      return const SizedBox.shrink();
    }
    final state = session?.contactId == value.contactId
        ? session!.typedState
        : value.typedState;
    final colors = Theme.of(context).colorScheme;
    final color = switch (state) {
      RadioState.transmitting => colors.primary,
      RadioState.startingCapture => colors.secondary,
      RadioState.receiving => colors.tertiary,
      RadioState.requestingFloor || RadioState.connecting => colors.secondary,
      RadioState.reconnecting || RadioState.unavailable => colors.error,
      RadioState.ready => colors.primary,
      _ => colors.outline,
    };
    final label = switch (state) {
      RadioState.ready => context.l10n.radioReady,
      RadioState.transmitting => context.l10n.radioTransmitting,
      RadioState.receiving => context.l10n.radioReceiving(
        contactName ?? context.l10n.contactLabel,
      ),
      RadioState.requestingFloor => context.l10n.radioRequestingFloor,
      RadioState.startingCapture => context.l10n.radioRequestingFloor,
      RadioState.connecting => context.l10n.radioConnecting,
      RadioState.reconnecting => context.l10n.radioReconnecting,
      RadioState.available ||
      RadioState.waitingForPeer => context.l10n.radioWaitingForPeer,
      _ => context.l10n.radioUnavailable,
    };
    return Tooltip(
      message: label,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 4),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(context.torcaIcons.radio, size: 17, color: color),
            const SizedBox(width: 4),
            Semantics(
              label: label,
              child: DecoratedBox(
                decoration: BoxDecoration(color: color, shape: BoxShape.circle),
                child: const SizedBox.square(dimension: 6),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
