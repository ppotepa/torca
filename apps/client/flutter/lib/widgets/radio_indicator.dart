import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';

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
      RadioState.ready => context.strings.radioReady,
      RadioState.transmitting => context.strings.radioTransmitting,
      RadioState.receiving => context.strings.radioReceiving(
        contactName ?? context.strings.contactLabel,
      ),
      RadioState.requestingFloor => context.strings.radioRequestingFloor,
      RadioState.startingCapture => context.strings.radioRequestingFloor,
      RadioState.connecting => context.strings.radioConnecting,
      RadioState.reconnecting => context.strings.radioReconnecting,
      RadioState.available ||
      RadioState.waitingForPeer => context.strings.radioWaitingForPeer,
      _ => context.strings.radioUnavailable,
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
