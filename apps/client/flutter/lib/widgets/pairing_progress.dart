import 'package:flutter/material.dart';

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
    final terminalFailure = const {
      'rejected',
      'cancelled',
      'expired',
    }.contains(state);
    return Semantics(
      label: terminalFailure
          ? 'Pairing ${state.toLowerCase()}'
          : 'Pairing step ${current + 1} of ${_steps.length}: ${_steps[current]}',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          LinearProgressIndicator(
            value: terminalFailure ? 0 : (current + 1) / _steps.length,
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 10,
            runSpacing: 6,
            children: List<Widget>.generate(_steps.length, (index) {
              final reached = !terminalFailure && index <= current;
              return Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Icon(
                    reached ? Icons.check_circle : Icons.circle_outlined,
                    size: 15,
                    color: reached
                        ? Theme.of(context).colorScheme.primary
                        : Theme.of(context).colorScheme.outline,
                  ),
                  const SizedBox(width: 4),
                  Text(
                    _steps[index],
                    style: Theme.of(context).textTheme.labelSmall,
                  ),
                ],
              );
            }),
          ),
        ],
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
