import 'dart:convert';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/diagnostics_overview.dart';
import '../widgets/runtime_network_status.dart';

class DiagnosticsScreen extends StatefulWidget {
  const DiagnosticsScreen({required this.gateway, super.key});
  final EngineGateway gateway;
  @override
  State<DiagnosticsScreen> createState() => _DiagnosticsScreenState();
}

class _DiagnosticsScreenState extends State<DiagnosticsScreen> {
  String? _json;
  String? _error;
  bool _loading = false;

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  Future<void> _refresh() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final value = await widget.gateway.diagnosticsJson();
      if (mounted) setState(() => _json = value);
    } on Object catch (error) {
      if (mounted) setState(() => _error = '$error');
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _export() async {
    final value = _json ?? await widget.gateway.diagnosticsJson();
    final path = await FilePicker.saveFile(
      dialogTitle: 'Export Torca diagnostics',
      fileName: 'torca-diagnostics.json',
    );
    if (path == null || !mounted) return;
    try {
      await File(path).writeAsString(value, flush: true);
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(const SnackBar(content: Text('Diagnostics exported')));
      }
    } on FileSystemException {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(const SnackBar(content: Text('Export failed')));
      }
    }
  }

  Future<void> _selfTest() async {
    await _refresh();
    if (!mounted) return;
    final snapshot = widget.gateway.snapshots.value;
    final checks = <_Check>[
      const _Check(
        'Native bridge snapshot',
        true,
        'Contract $torcaContractVersion snapshot is readable',
      ),
      _Check(
        'Local identity',
        snapshot.identity != null,
        snapshot.identity == null
            ? 'Identity is not initialized'
            : 'Identity loaded',
      ),
      _Check(
        'Embedded Tor',
        snapshot.torState == 'ready',
        'Tor state: ${snapshot.torState}',
      ),
      _Check(
        'Onion service',
        (snapshot.onionAddress ?? '').endsWith('.onion'),
        snapshot.onionAddress ?? 'No onion address',
      ),
      _Check(
        'Diagnostics stream',
        _hasReadableEvents(_json),
        _hasReadableEvents(_json)
            ? 'Health events readable'
            : 'No readable health events',
      ),
    ];
    await showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Connection self-test'),
        content: SizedBox(
          width: 520,
          child: ListView(
            shrinkWrap: true,
            children: checks
                .map(
                  (check) => ListTile(
                    leading: Icon(
                      check.ok
                          ? Icons.check_circle_outline
                          : Icons.error_outline,
                    ),
                    title: Text(check.name),
                    subtitle: Text(check.detail),
                  ),
                )
                .toList(growable: false),
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }

  bool _hasReadableEvents(String? value) {
    if (value == null || value.isEmpty) return false;
    try {
      final decoded = jsonDecode(value);
      return decoded is Map && decoded['events'] is List;
    } on FormatException {
      return false;
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: RuntimeAppBar(
      title: const Text('Diagnostics'),
      actions: <Widget>[
        IconButton(
          tooltip: 'Run self-test',
          onPressed: _loading ? null : _selfTest,
          icon: const Icon(Icons.fact_check_outlined),
        ),
        IconButton(
          tooltip: 'Export diagnostics',
          onPressed: _loading ? null : _export,
          icon: const Icon(Icons.save_alt),
        ),
        IconButton(
          tooltip: 'Refresh',
          onPressed: _loading ? null : _refresh,
          icon: const Icon(Icons.refresh),
        ),
      ],
    ),
    body: _loading && _json == null
        ? const Center(child: CircularProgressIndicator())
        : _error != null
        ? Center(
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Text(_error!),
            ),
          )
        : ValueListenableBuilder<AppSnapshotDto>(
            valueListenable: widget.gateway.snapshots,
            builder: (context, snapshot, _) => ListView(
              padding: const EdgeInsets.all(16),
              children: <Widget>[
                Text(
                  'Health overview',
                  style: Theme.of(context).textTheme.titleLarge,
                ),
                const SizedBox(height: 12),
                DiagnosticsOverview(
                  snapshot: snapshot,
                  diagnosticsReadable: _hasReadableEvents(_json),
                ),
                const SizedBox(height: 20),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    FilledButton.tonalIcon(
                      onPressed: _loading ? null : _selfTest,
                      icon: const Icon(Icons.fact_check_outlined),
                      label: const Text('Run self-test'),
                    ),
                    OutlinedButton.icon(
                      onPressed: _loading ? null : _export,
                      icon: const Icon(Icons.save_alt),
                      label: const Text('Export diagnostics'),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                ExpansionTile(
                  tilePadding: EdgeInsets.zero,
                  title: const Text('Raw diagnostics'),
                  subtitle: const Text('Redacted developer event stream'),
                  children: <Widget>[
                    Container(
                      width: double.infinity,
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: Theme.of(
                          context,
                        ).colorScheme.surfaceContainerLow,
                        borderRadius: BorderRadius.circular(10),
                      ),
                      child: SelectableText(_pretty(_json ?? '{"events":[]}')),
                    ),
                  ],
                ),
              ],
            ),
          ),
  );

  String _pretty(String value) {
    try {
      return const JsonEncoder.withIndent('  ').convert(jsonDecode(value));
    } on FormatException {
      return value;
    }
  }
}

class _Check {
  const _Check(this.name, this.ok, this.detail);
  final String name;
  final bool ok;
  final String detail;
}
