import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../controllers/pairing_action_controller.dart';
import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';

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
  late final PairingActionController _actions = PairingActionController(
    widget.gateway,
  )..addListener(_changed);
  void _changed() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _actions
      ..removeListener(_changed)
      ..dispose();
    super.dispose();
  }

  PairingDto? _current(AppSnapshotDto snapshot) {
    for (final pairing in snapshot.pairings) {
      if (pairing.id == widget.pairing.id) return pairing;
    }
    return null;
  }

  Future<void> _run(PairingAction action, {required bool accept}) async {
    final succeeded = await _actions.run(action, widget.pairing.id);
    if (!mounted || !succeeded) return;
    // The command is durable: the runtime owns the remainder of the secure
    // exchange.  Keeping a second "Contact connected" view above the
    // invitation produces a modal cascade and traps users while the peer is
    // reconnecting.  Close this attention surface after either decision; the
    // app-level coordinator presents the one completion notification.
    Navigator.of(context).pop(accept);
  }

  @override
  Widget build(BuildContext context) => PopScope(
    // Network completion continues in the runtime; it must never trap the user
    // inside a modal while relay or peer connectivity is recovering.
    canPop: true,
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
            return SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(24, 22, 24, 20),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Icon(
                    context.torcaIcons.addContact,
                    size: 58,
                    color: Theme.of(context).colorScheme.primary,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    context.strings.newPairingRequest,
                    textAlign: TextAlign.center,
                    style: Theme.of(context).textTheme.headlineSmall,
                  ),
                  const SizedBox(height: 10),
                  _IdentitySummary(pairing: pairing ?? widget.pairing),
                  const SizedBox(height: 14),
                  Text(
                    context.strings.pairingRequestDescription,
                    textAlign: TextAlign.center,
                  ),
                  if (_actions.error(context) != null) ...<Widget>[
                    const SizedBox(height: 12),
                    Text(
                      _actions.error(context)!,
                      textAlign: TextAlign.center,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ],
                  const SizedBox(height: 20),
                  Row(
                    children: <Widget>[
                      Expanded(
                        child: FilledButton.icon(
                          onPressed: _actions.busy
                              ? null
                              : () => _run(PairingAction.approve, accept: true),
                          icon: _actions.busy
                              ? const SizedBox(
                                  width: 18,
                                  height: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : Icon(context.torcaIcons.confirm),
                          label: Text(context.strings.accept),
                        ),
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: OutlinedButton.icon(
                          onPressed: _actions.busy
                              ? null
                              : () => _run(PairingAction.reject, accept: false),
                          icon: Icon(context.torcaIcons.close),
                          label: Text(context.strings.reject),
                        ),
                      ),
                    ],
                  ),
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
        borderRadius: BorderRadius.circular(context.torcaTokens.radiusLarge),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            name == null || name.isEmpty ? context.strings.newDevice : name,
            style: Theme.of(context).textTheme.titleLarge,
          ),
          if (pairing.remoteIdentityId != null) ...<Widget>[
            const SizedBox(height: 6),
            Text(context.strings.remoteIdentity(pairing.remoteIdentityId)),
          ],
          if (pairing.remoteFingerprint != null) ...<Widget>[
            const SizedBox(height: 8),
            Text(
              pairing.remoteFingerprint!,
              style: context.torcaCodeStyle(
                Theme.of(context).textTheme.bodySmall,
              ),
            ),
          ],
        ],
      ),
    );
  }
}
