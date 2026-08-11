import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

class PairingProgress extends StatelessWidget {
  const PairingProgress({required this.state, super.key});

  final String state;

  static const _steps = <String>[
    'Invitation',
    'Peer joined',
    'Verify',
    'Approved',
    'P2P ready',
  ];

  @override
  Widget build(BuildContext context) {
    final current = _stage(state);
    final icons = <IconData>[
      context.torcaIcons.invitations,
      context.torcaIcons.link,
      context.torcaIcons.identity,
      context.torcaIcons.confirm,
      context.torcaIcons.online,
    ];
    final terminalFailure = const {
      'rejected',
      'cancelled',
      'expired',
    }.contains(state);
    return Semantics(
      label: terminalFailure
          ? 'Pairing ${state.toLowerCase()}'
          : 'Pairing step ${current + 1} of ${_steps.length}: ${_steps[current]}',
      child: Row(
        children: List<Widget>.generate(_steps.length * 2 - 1, (position) {
          if (position.isOdd) {
            return Expanded(
              child: Icon(
                context.torcaIcons.send,
                size: 18,
                color: Theme.of(context).colorScheme.outline,
              ),
            );
          }
          final index = position ~/ 2;
          final reached = !terminalFailure && index <= current;
          return Expanded(
            child: Tooltip(
              message: _steps[index],
              child: Icon(
                icons[index],
                size: 28,
                color: reached
                    ? Theme.of(context).colorScheme.primary
                    : Theme.of(context).colorScheme.outline,
              ),
            ),
          );
        }),
      ),
    );
  }

  static int _stage(String state) => switch (state) {
    'peerjoined' || 'peer_joined' => 1,
    'awaitingapproval' || 'awaiting_approval' => 2,
    'approved' => 3,
    'completed' => 4,
    _ => 0,
  };
}
