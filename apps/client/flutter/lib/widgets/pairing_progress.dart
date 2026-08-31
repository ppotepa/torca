import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';

class PairingProgress extends StatelessWidget {
  const PairingProgress({required this.state, super.key});

  final String state;

  @override
  Widget build(BuildContext context) {
    final typedState = pairingStateFromWire(state);
    final stage = _stage(typedState);
    final terminalFailure = switch (typedState) {
      PairingState.rejected ||
      PairingState.cancelled ||
      PairingState.expired => true,
      _ => false,
    };
    final colors = Theme.of(context).colorScheme;
    final icon = terminalFailure
        ? context.torcaIcons.warning
        : typedState == PairingState.completed
        ? context.torcaIcons.success
        : context.torcaIcons.invitations;
    final label = context.strings.pairingStateLabel(typedState);
    return Semantics(
      label: label,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(
                icon,
                size: 20,
                color: terminalFailure ? colors.error : colors.primary,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  label,
                  style: Theme.of(context).textTheme.labelLarge,
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          LinearProgressIndicator(
            value: terminalFailure ? 1 : stage / 4,
            color: terminalFailure ? colors.error : colors.primary,
            backgroundColor: colors.surfaceContainerHighest,
          ),
        ],
      ),
    );
  }

  static int _stage(PairingState state) => switch (state) {
    PairingState.peerJoined => 1,
    PairingState.awaitingApproval => 2,
    PairingState.approved => 3,
    PairingState.completed => 4,
    _ => 0,
  };
}
