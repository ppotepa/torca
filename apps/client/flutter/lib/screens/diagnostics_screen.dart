import 'dart:convert';

import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

class DiagnosticsScreen extends StatefulWidget {
  const DiagnosticsScreen({required this.gateway, super.key});
  final EngineGateway gateway;
  @override
  State<DiagnosticsScreen> createState() => _DiagnosticsScreenState();
}

class _DiagnosticsScreenState extends State<DiagnosticsScreen> {
  late Future<String> _diagnostics = widget.gateway.diagnosticsJson();

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(
      title: const Text('Diagnostics'),
      actions: <Widget>[
        IconButton(
          tooltip: 'Refresh diagnostics',
          onPressed: () => setState(() => _diagnostics = widget.gateway.diagnosticsJson()),
          icon: const Icon(Icons.refresh),
        ),
      ],
    ),
    body: ValueListenableBuilder<AppSnapshotDto>(
      valueListenable: widget.gateway.snapshots,
      builder: (context, snapshot, _) => FutureBuilder<String>(
        future: _diagnostics,
        builder: (context, async) {
          final raw = async.data ?? '{"events":[]}';
          final events = _events(raw);
          return ListView(
            padding: const EdgeInsets.all(24),
            children: <Widget>[
              ListTile(
                leading: const Icon(Icons.security),
                title: const Text('Tor'),
                subtitle: Text('${snapshot.torState}${snapshot.onionAddress == null ? '' : ' · onion ready'}'),
              ),
              ListTile(
                leading: const Icon(Icons.hub),
                title: const Text('Direct peers'),
                subtitle: Text('${snapshot.contacts.where((contact) => contact.connectionState == 'ready').length}/${snapshot.contacts.length} ready'),
              ),
              const ListTile(
                leading: Icon(Icons.visibility_off_outlined),
                title: Text('Sensitive data'),
                subtitle: Text('Private keys, pairwise secrets, capabilities and plaintext diagnostics are never exported.'),
              ),
              const Divider(),
              Text('Runtime events', style: Theme.of(context).textTheme.titleMedium),
              const SizedBox(height: 8),
              if (async.connectionState == ConnectionState.waiting)
                const Center(child: CircularProgressIndicator())
              else if (events.isEmpty)
                const Text('No diagnostic events recorded yet.')
              else
                ...events.map((event) => ListTile(
                  dense: true,
                  leading: const Icon(Icons.circle, size: 10),
                  title: Text('${event['component'] ?? 'Runtime'} · ${event['state'] ?? 'unknown'}'),
                  subtitle: Text('${event['code'] ?? ''}${event['detail'] == null ? '' : '\n${event['detail']}'}'),
                )),
              const Divider(),
              ExpansionTile(
                title: const Text('Redacted JSON export'),
                children: <Widget>[
                  Padding(
                    padding: const EdgeInsets.all(12),
                    child: SelectableText(raw),
                  ),
                ],
              ),
            ],
          );
        },
      ),
    ),
  );

  List<Map<String, Object?>> _events(String raw) {
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map<String, Object?>) return const [];
      final value = decoded['events'];
      if (value is! List) return const [];
      return value.whereType<Map>().map((item) => item.map(
        (key, value) => MapEntry(key.toString(), value),
      )).toList(growable: false);
    } catch (_) {
      return const [];
    }
  }
}
