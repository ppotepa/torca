import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

/// Global approval surface for a remote device that joined an invitation.
class IncomingPairingDialog extends StatefulWidget {
  const IncomingPairingDialog({
    required this.gateway,
    required this.pairing,
    super.key,
  });

  final EngineGateway gateway;
  final PairingDto pairing;

  @override
  State<IncomingPairingDialog> createState() => _IncomingPairingDialogState();
}

class _IncomingPairingDialogState extends State<IncomingPairingDialog> {
  bool _busy = false;
  bool _accepted = false;
  String? _error;

  PairingDto? _current(AppSnapshotDto snapshot) {
    for (final pairing in snapshot.pairings) {
      if (pairing.id == widget.pairing.id) return pairing;
    }
    return null;
  }

  Future<void> _run(BridgeCommandDto command, {required bool accept}) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final result = await widget.gateway.execute(command);
    if (!mounted) return;
    if (!result.ok) {
      setState(() {
        _busy = false;
        _error = result.error ?? 'Pairing operation failed';
      });
      return;
    }
    if (!accept) {
      Navigator.of(context).pop();
      return;
    }
    setState(() {
      _busy = false;
      _accepted = true;
    });
  }

  @override
  Widget build(BuildContext context) => PopScope(
    canPop: !_busy,
    child: Dialog(
      insetPadding: const EdgeInsets.symmetric(horizontal: 18, vertical: 24),
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxWidth: 480,
          maxHeight: MediaQuery.sizeOf(context).height - 48,
        ),
        child: ValueListenableBuilder<AppSnapshotDto>(
          valueListenable: widget.gateway.snapshots,
          builder: (context, snapshot, _) {
            final pairing = _current(snapshot);
            final completed =
                _accepted &&
                (pairing == null ||
                    pairing.typedState == PairingState.completed);
            return SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(24, 22, 24, 20),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Icon(
                    completed ? Icons.check_circle : Icons.person_add_alt_1,
                    size: 58,
                    color: completed
                        ? Colors.green.shade700
                        : Theme.of(context).colorScheme.primary,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    completed ? 'Contact connected' : 'New pairing request',
                    textAlign: TextAlign.center,
                    style: Theme.of(context).textTheme.headlineSmall,
                  ),
                  const SizedBox(height: 10),
                  if (completed) ...<Widget>[
                    const Text(
                      'The invitation was accepted and the private conversation is ready.',
                      textAlign: TextAlign.center,
                    ),
                    const SizedBox(height: 20),
                    FilledButton.icon(
                      onPressed: () => Navigator.of(context).pop(),
                      icon: const Icon(Icons.check),
                      label: const Text('Done'),
                    ),
                  ] else ...<Widget>[
                    _IdentitySummary(pairing: pairing ?? widget.pairing),
                    const SizedBox(height: 14),
                    Text(
                      _accepted
                          ? 'Decision saved. Waiting for the secure connection to finish.'
                          : 'This device joined your invitation. Verify the identity before accepting.',
                      textAlign: TextAlign.center,
                    ),
                    if (_error != null) ...<Widget>[
                      const SizedBox(height: 12),
                      Text(
                        _error!,
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ),
                    ],
                    const SizedBox(height: 20),
                    if (!_accepted)
                      Row(
                        children: <Widget>[
                          Expanded(
                            child: FilledButton.icon(
                              onPressed: _busy
                                  ? null
                                  : () => _run(
                                      ApprovePairingCommandDto(
                                        sessionIdHex: widget.pairing.id,
                                      ),
                                      accept: true,
                                    ),
                              icon: _busy
                                  ? const SizedBox(
                                      width: 18,
                                      height: 18,
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    )
                                  : const Icon(Icons.check),
                              label: const Text('Accept'),
                            ),
                          ),
                          const SizedBox(width: 10),
                          Expanded(
                            child: OutlinedButton.icon(
                              onPressed: _busy
                                  ? null
                                  : () => _run(
                                      RejectPairingCommandDto(
                                        sessionIdHex: widget.pairing.id,
                                      ),
                                      accept: false,
                                    ),
                              icon: const Icon(Icons.close),
                              label: const Text('Reject'),
                            ),
                          ),
                        ],
                      )
                    else
                      OutlinedButton(
                        onPressed: _busy
                            ? null
                            : () => Navigator.of(context).pop(),
                        child: const Text('Close'),
                      ),
                  ],
                ],
              ),
            );
          },
        ),
      ),
    ),
  );
}

class _IdentitySummary extends StatelessWidget {
  const _IdentitySummary({required this.pairing});
  final PairingDto pairing;

  @override
  Widget build(BuildContext context) {
    final name = pairing.remoteDisplayName?.trim();
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(14),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            name == null || name.isEmpty ? 'New device' : name,
            style: Theme.of(context).textTheme.titleLarge,
          ),
          if (pairing.remoteIdentityId != null) ...<Widget>[
            const SizedBox(height: 6),
            Text('Identity ${pairing.remoteIdentityId}'),
          ],
          if (pairing.remoteFingerprint != null) ...<Widget>[
            const SizedBox(height: 8),
            Text(
              pairing.remoteFingerprint!,
              style: Theme.of(
                context,
              ).textTheme.bodySmall?.copyWith(fontFamily: 'monospace'),
            ),
          ],
        ],
      ),
    );
  }
}
