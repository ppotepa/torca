import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

enum _PairingMode { create, join }

class PairingScreen extends StatefulWidget {
  const PairingScreen({required this.gateway, super.key});

  final EngineGateway gateway;

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final TextEditingController controller = TextEditingController();
  _PairingMode _mode = _PairingMode.join;
  String? error;
  bool _submitting = false;

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Pair contact')),
        body: ValueListenableBuilder<AppSnapshotDto>(
          valueListenable: widget.gateway.snapshots,
          builder: (
            BuildContext context,
            AppSnapshotDto snapshot,
            Widget? child,
          ) {
            return ListView(
              padding: const EdgeInsets.all(24),
              children: <Widget>[
                Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 520),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        SegmentedButton<_PairingMode>(
                          segments: const <ButtonSegment<_PairingMode>>[
                            ButtonSegment<_PairingMode>(
                              value: _PairingMode.create,
                              icon: Icon(Icons.add_link),
                              label: Text('Create invitation'),
                            ),
                            ButtonSegment<_PairingMode>(
                              value: _PairingMode.join,
                              icon: Icon(Icons.link),
                              label: Text('Join invitation'),
                            ),
                          ],
                          selected: <_PairingMode>{_mode},
                          onSelectionChanged: _submitting
                              ? null
                              : (Set<_PairingMode> value) {
                                  setState(() {
                                    _mode = value.single;
                                    error = null;
                                  });
                                },
                        ),
                        const SizedBox(height: 20),
                        TextField(
                          controller: controller,
                          enabled: !_submitting,
                          textCapitalization: TextCapitalization.characters,
                          autocorrect: false,
                          enableSuggestions: false,
                          decoration: InputDecoration(
                            labelText: _mode == _PairingMode.create
                                ? 'Invitation code'
                                : 'Pairing code',
                            helperText: _mode == _PairingMode.create
                                ? 'Choose a temporary 6–16 character code to share.'
                                : 'Enter the temporary code shown by your contact.',
                            errorText: error,
                            border: const OutlineInputBorder(),
                          ),
                          onSubmitted: _submitting ? null : (_) => _submit(),
                        ),
                        const SizedBox(height: 16),
                        FilledButton.icon(
                          onPressed: _submitting ? null : _submit,
                          icon: Icon(
                            _mode == _PairingMode.create
                                ? Icons.add_link
                                : Icons.link,
                          ),
                          label: Text(
                            _submitting
                                ? 'Working…'
                                : _mode == _PairingMode.create
                                    ? 'Create invitation'
                                    : 'Join invitation',
                          ),
                        ),
                        if (snapshot.pairings.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 32),
                          Text(
                            'Pairing sessions',
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                          const SizedBox(height: 12),
                          ...snapshot.pairings.map(
                            (PairingDto pairing) => _PairingSessionCard(
                              pairing: pairing,
                              busy: _submitting,
                              onApprove: () => _approve(pairing),
                              onReject: () => _reject(pairing),
                              onCancel: () => _cancel(pairing),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
              ],
            );
          },
        ),
      );

  Future<void> _submit() async {
    final String code = controller.text.trim().toUpperCase();
    if (!RegExp(r'^[A-Z0-9]{6,16}$').hasMatch(code)) {
      setState(() {
        error = 'Use 6–16 letters or digits';
      });
      return;
    }

    setState(() {
      _submitting = true;
      error = null;
    });
    final int now = DateTime.now().microsecondsSinceEpoch;
    final String id = now.toRadixString(16).padLeft(32, '0').substring(0, 32);
    final int expiresAtMs = DateTime.now()
        .add(const Duration(minutes: 5))
        .millisecondsSinceEpoch;
    final BridgeCommandDto command = _mode == _PairingMode.create
        ? StartPairingCommandDto(
            sessionIdHex: id,
            code: code,
            expiresAtMs: expiresAtMs,
          )
        : JoinPairingCommandDto(
            sessionIdHex: id,
            code: code,
            expiresAtMs: expiresAtMs,
          );
    final BridgeResultDto result = await widget.gateway.execute(command);
    if (!mounted) return;
    setState(() {
      _submitting = false;
      if (result.ok) {
        controller.clear();
      } else {
        error = result.error ?? 'Pairing operation failed';
      }
    });
  }

  Future<void> _approve(PairingDto pairing) => _executeSessionCommand(
        ApprovePairingCommandDto(
          sessionIdHex: pairing.id,
          atMs: DateTime.now().millisecondsSinceEpoch,
        ),
      );

  Future<void> _reject(PairingDto pairing) => _executeSessionCommand(
        RejectPairingCommandDto(sessionIdHex: pairing.id),
      );

  Future<void> _cancel(PairingDto pairing) => _executeSessionCommand(
        CancelPairingCommandDto(sessionIdHex: pairing.id),
      );

  Future<void> _executeSessionCommand(BridgeCommandDto command) async {
    setState(() {
      _submitting = true;
      error = null;
    });
    final BridgeResultDto result = await widget.gateway.execute(command);
    if (!mounted) return;
    setState(() {
      _submitting = false;
      if (!result.ok) error = result.error ?? 'Pairing operation failed';
    });
  }
}

class _PairingSessionCard extends StatelessWidget {
  const _PairingSessionCard({
    required this.pairing,
    required this.busy,
    required this.onApprove,
    required this.onReject,
    required this.onCancel,
  });

  final PairingDto pairing;
  final bool busy;
  final VoidCallback onApprove;
  final VoidCallback onReject;
  final VoidCallback onCancel;

  bool get _terminal => const <String>{
        'rejected',
        'cancelled',
        'expired',
        'completed',
      }.contains(pairing.state);

  @override
  Widget build(BuildContext context) {
    final bool canReview = pairing.state == 'awaitingapproval';
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    pairing.code,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                Chip(label: Text(pairing.state)),
              ],
            ),
            Text('${pairing.role} · expires ${_expiry(pairing.expiresAtMs)}'),
            if (canReview) ...<Widget>[
              const SizedBox(height: 12),
              const Text('The peer is ready for explicit approval.'),
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                children: <Widget>[
                  FilledButton(
                    onPressed: busy ? null : onApprove,
                    child: const Text('Approve'),
                  ),
                  OutlinedButton(
                    onPressed: busy ? null : onReject,
                    child: const Text('Reject'),
                  ),
                ],
              ),
            ] else if (!_terminal) ...<Widget>[
              const SizedBox(height: 8),
              TextButton(
                onPressed: busy ? null : onCancel,
                child: const Text('Cancel'),
              ),
            ],
          ],
        ),
      ),
    );
  }

  String _expiry(int milliseconds) {
    final DateTime value = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    final String minute = value.minute.toString().padLeft(2, '0');
    return '${value.hour}:$minute';
  }
}
