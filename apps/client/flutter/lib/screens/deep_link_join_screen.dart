import 'dart:math';

import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import 'pairing_screen.dart';

class DeepLinkJoinScreen extends StatefulWidget {
  const DeepLinkJoinScreen({required this.gateway, required this.code, super.key});
  final EngineGateway gateway;
  final String code;
  @override State<DeepLinkJoinScreen> createState() => _DeepLinkJoinScreenState();
}

class _DeepLinkJoinScreenState extends State<DeepLinkJoinScreen> {
  final Random _random = Random.secure();
  bool _busy = false;
  String? _error;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('Join Torca invitation')),
    body: Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 480),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              const Icon(Icons.link, size: 48),
              const SizedBox(height: 16),
              const Text('Invitation code', textAlign: TextAlign.center),
              const SizedBox(height: 8),
              SelectableText(widget.code, textAlign: TextAlign.center, style: Theme.of(context).textTheme.headlineSmall),
              const SizedBox(height: 20),
              FilledButton(onPressed: _busy ? null : _join, child: Text(_busy ? 'Joining…' : 'Join invitation')),
              if (_error != null) ...<Widget>[
                const SizedBox(height: 12),
                Text(_error!, style: TextStyle(color: Theme.of(context).colorScheme.error)),
              ],
            ],
          ),
        ),
      ),
    ),
  );

  Future<void> _join() async {
    setState(() { _busy = true; _error = null; });
    final result = await widget.gateway.execute(JoinPairingCommandDto(sessionIdHex: _newId(), code: widget.code));
    if (!mounted) return;
    if (result.ok) {
      await Navigator.of(context).pushReplacement<void, void>(MaterialPageRoute(builder: (_) => PairingScreen(gateway: widget.gateway)));
    } else {
      setState(() { _busy = false; _error = result.error ?? 'Could not join invitation'; });
    }
  }

  String _newId() {
    final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
    if (bytes.every((value) => value == 0)) bytes[15] = 1;
    return bytes.map((value) => value.toRadixString(16).padLeft(2, '0')).join();
  }
}
