import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

class PairingScreen extends StatefulWidget {
  const PairingScreen({required this.gateway, super.key});

  final EngineGateway gateway;

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final TextEditingController controller = TextEditingController(text: 'TORCA1');
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
        body: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  TextField(
                    controller: controller,
                    enabled: !_submitting,
                    textCapitalization: TextCapitalization.characters,
                    decoration: InputDecoration(
                      labelText: 'Pairing code',
                      errorText: error,
                      border: const OutlineInputBorder(),
                    ),
                  ),
                  const SizedBox(height: 16),
                  FilledButton(
                    onPressed: _submitting ? null : _startPairing,
                    child: Text(_submitting ? 'Pairing…' : 'Start pairing'),
                  ),
                ],
              ),
            ),
          ),
        ),
      );

  Future<void> _startPairing() async {
    final String code = controller.text.trim();
    if (code.isEmpty) {
      setState(() {
        error = 'Pairing code is required';
      });
      return;
    }

    setState(() {
      _submitting = true;
      error = null;
    });
    final String id = DateTime.now()
        .microsecondsSinceEpoch
        .toRadixString(16)
        .padLeft(32, '0')
        .substring(0, 32);
    final BridgeResultDto result = await widget.gateway.execute(
      StartPairingCommandDto(
        sessionIdHex: id,
        code: code,
        expiresAtMs: DateTime.now()
            .add(const Duration(minutes: 5))
            .millisecondsSinceEpoch,
      ),
    );
    if (!mounted) {
      return;
    }
    if (result.ok) {
      Navigator.of(context).pop();
      return;
    }
    setState(() {
      _submitting = false;
      error = result.error ?? 'Pairing could not be started';
    });
  }
}
