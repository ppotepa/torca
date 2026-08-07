import 'dart:math';

import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:qr_flutter/qr_flutter.dart';

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
  final TextEditingController _code = TextEditingController();
  final Random _random = Random.secure();
  _PairingMode _mode = _PairingMode.create;
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _code.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('Pair contact')),
    body: ValueListenableBuilder<AppSnapshotDto>(
      valueListenable: widget.gateway.snapshots,
      builder: (context, snapshot, _) => ListView(
        padding: const EdgeInsets.all(24),
        children: <Widget>[
          Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 560),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  SegmentedButton<_PairingMode>(
                    segments: const <ButtonSegment<_PairingMode>>[
                      ButtonSegment(value: _PairingMode.create, icon: Icon(Icons.qr_code_2), label: Text('Create invitation')),
                      ButtonSegment(value: _PairingMode.join, icon: Icon(Icons.qr_code_scanner), label: Text('Join invitation')),
                    ],
                    selected: <_PairingMode>{_mode},
                    onSelectionChanged: _busy ? null : (value) => setState(() {
                      _mode = value.single;
                      _error = null;
                    }),
                  ),
                  const SizedBox(height: 20),
                  if (_mode == _PairingMode.create) ...<Widget>[
                    const Text('Torca generates a short-lived invitation code in the Rust runtime.'),
                    const SizedBox(height: 12),
                    FilledButton.icon(
                      onPressed: _busy ? null : _create,
                      icon: const Icon(Icons.add_link),
                      label: Text(_busy ? 'Creating…' : 'Create invitation'),
                    ),
                  ] else ...<Widget>[
                    TextField(
                      controller: _code,
                      enabled: !_busy,
                      textCapitalization: TextCapitalization.characters,
                      autocorrect: false,
                      enableSuggestions: false,
                      decoration: InputDecoration(
                        labelText: 'Pairing code or Torca QR URI',
                        helperText: 'Enter the code or scan the QR shown by your contact.',
                        errorText: _error,
                        border: const OutlineInputBorder(),
                        suffixIcon: IconButton(
                          tooltip: 'Scan QR',
                          onPressed: _busy ? null : _scan,
                          icon: const Icon(Icons.qr_code_scanner),
                        ),
                      ),
                      onSubmitted: _busy ? null : (_) => _join(),
                    ),
                    const SizedBox(height: 12),
                    FilledButton.icon(
                      onPressed: _busy ? null : _join,
                      icon: const Icon(Icons.link),
                      label: Text(_busy ? 'Joining…' : 'Join invitation'),
                    ),
                  ],
                  if (_error != null && _mode == _PairingMode.create) ...<Widget>[
                    const SizedBox(height: 12),
                    Text(_error!, style: TextStyle(color: Theme.of(context).colorScheme.error)),
                  ],
                  if (snapshot.pairings.isNotEmpty) ...<Widget>[
                    const SizedBox(height: 32),
                    Text('Pairing sessions', style: Theme.of(context).textTheme.titleMedium),
                    const SizedBox(height: 12),
                    ...snapshot.pairings.reversed.map((pairing) => _PairingSessionCard(
                      pairing: pairing,
                      busy: _busy,
                      onApprove: () => _session(ApprovePairingCommandDto(sessionIdHex: pairing.id)),
                      onReject: () => _session(RejectPairingCommandDto(sessionIdHex: pairing.id)),
                      onCancel: () => _session(CancelPairingCommandDto(sessionIdHex: pairing.id)),
                    )),
                  ],
                ],
              ),
            ),
          ),
        ],
      ),
    ),
  );

  Future<void> _create() async {
    await _execute(CreatePairingCommandDto(sessionIdHex: _newId()));
  }

  Future<void> _join() async {
    final code = _extractCode(_code.text);
    if (code == null) {
      setState(() => _error = 'Use a 6–16 character code or a valid Torca QR URI');
      return;
    }
    final result = await _execute(JoinPairingCommandDto(sessionIdHex: _newId(), code: code));
    if (result.ok && mounted) _code.clear();
  }

  Future<void> _scan() async {
    final scanned = await showDialog<String>(
      context: context,
      builder: (context) => Dialog(
        child: SizedBox(
          width: 420,
          height: 520,
          child: Stack(
            children: <Widget>[
              MobileScanner(
                onDetect: (capture) {
                  for (final barcode in capture.barcodes) {
                    final value = barcode.rawValue;
                    if (value != null && _extractCode(value) != null) {
                      Navigator.of(context).pop(value);
                      return;
                    }
                  }
                },
              ),
              Positioned(
                right: 8,
                top: 8,
                child: IconButton.filledTonal(
                  onPressed: () => Navigator.of(context).pop(),
                  icon: const Icon(Icons.close),
                ),
              ),
            ],
          ),
        ),
      ),
    );
    if (scanned == null || !mounted) return;
    _code.text = _extractCode(scanned) ?? '';
    await _join();
  }

  Future<BridgeResultDto> _execute(BridgeCommandDto command) async {
    setState(() { _busy = true; _error = null; });
    final result = await widget.gateway.execute(command);
    if (mounted) setState(() {
      _busy = false;
      if (!result.ok) _error = result.error ?? 'Pairing operation failed';
    });
    return result;
  }

  Future<void> _session(BridgeCommandDto command) async { await _execute(command); }

  String _newId() {
    final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
    if (bytes.every((value) => value == 0)) bytes[15] = 1;
    return bytes.map((value) => value.toRadixString(16).padLeft(2, '0')).join();
  }

  String? _extractCode(String input) {
    final value = input.trim();
    final direct = value.toUpperCase();
    if (RegExp(r'^[A-Z0-9]{6,16}$').hasMatch(direct)) return direct;
    final uri = Uri.tryParse(value);
    if (uri == null || uri.scheme != 'torca' || uri.host != 'pair' || uri.queryParameters['v'] != '1') return null;
    final code = uri.queryParameters['code']?.toUpperCase();
    return code != null && RegExp(r'^[A-Z0-9]{6,16}$').hasMatch(code) ? code : null;
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

  bool get _terminal => const {'rejected', 'cancelled', 'expired', 'completed'}.contains(pairing.state);
  String get _uri => 'torca://pair?v=1&code=${Uri.encodeQueryComponent(pairing.code)}';

  @override
  Widget build(BuildContext context) {
    final canReview = pairing.state == 'awaitingapproval' || pairing.state == 'awaiting_approval';
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(children: <Widget>[
              Expanded(child: Text(pairing.code, style: Theme.of(context).textTheme.titleMedium)),
              Chip(label: Text(pairing.state)),
            ]),
            Text('${pairing.role} · expires ${_expiry(pairing.expiresAtMs)}'),
            if (pairing.role == 'creator' && !_terminal) ...<Widget>[
              const SizedBox(height: 12),
              Center(child: QrImageView(data: _uri, size: 190, semanticsLabel: 'Torca pairing invitation')),
              const SizedBox(height: 8),
              SelectableText(_uri, textAlign: TextAlign.center),
            ],
            if (canReview) ...<Widget>[
              const SizedBox(height: 12),
              const Text('The peer is ready for explicit approval.'),
              const SizedBox(height: 8),
              Wrap(spacing: 8, children: <Widget>[
                FilledButton(onPressed: busy ? null : onApprove, child: const Text('Approve')),
                OutlinedButton(onPressed: busy ? null : onReject, child: const Text('Reject')),
              ]),
            ] else if (!_terminal) ...<Widget>[
              const SizedBox(height: 8),
              TextButton(onPressed: busy ? null : onCancel, child: const Text('Cancel')),
            ],
          ],
        ),
      ),
    );
  }

  String _expiry(int milliseconds) {
    final value = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    return '${value.hour}:${value.minute.toString().padLeft(2, '0')}';
  }
}
