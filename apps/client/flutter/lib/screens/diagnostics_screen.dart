import 'dart:convert';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
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
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(context.strings.diagnosticsExported)),
        );
      }
    } on FileSystemException {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(context.strings.exportFailed)));
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
        snapshot.transport.tor.typedState == TransportState.ready,
        'Tor state: ${snapshot.torState}',
      ),
      _Check(
        'Onion service',
        (snapshot.onionAddress ?? '').endsWith('.onion'),
        snapshot.onionAddress ?? 'No onion address',
      ),
      _Check(
        context.strings.diagnosticsStream,
        _hasReadableEvents(_json),
        _hasReadableEvents(_json)
            ? 'Health events readable'
            : 'No readable health events',
      ),
    ];
    await showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(context.strings.connectionSelfTest),
        content: SizedBox(
          width: 520,
          child: ListView(
            shrinkWrap: true,
            children: checks
                .map(
                  (check) => ListTile(
                    leading: Icon(
                      check.ok
                          ? context.torcaIcons.success
                          : context.torcaIcons.error,
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
            child: Text(context.strings.close),
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
      title: Text(context.strings.diagnostics),
      actions: <Widget>[
        IconButton(
          tooltip: context.strings.runSelfTest,
          onPressed: _loading ? null : _selfTest,
          icon: Icon(context.torcaIcons.diagnostics),
        ),
        IconButton(
          tooltip: context.strings.exportDiagnostics,
          onPressed: _loading ? null : _export,
          icon: Icon(context.torcaIcons.save),
        ),
        IconButton(
          tooltip: context.strings.refresh,
          onPressed: _loading ? null : _refresh,
          icon: Icon(context.torcaIcons.retry),
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
                      icon: Icon(context.torcaIcons.diagnostics),
                      label: Text(context.strings.runSelfTest),
                    ),
                    OutlinedButton.icon(
                      onPressed: _loading ? null : _export,
                      icon: Icon(context.torcaIcons.save),
                      label: Text(context.strings.exportDiagnostics),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                ExpansionTile(
                  tilePadding: EdgeInsets.zero,
                  title: Text(context.strings.rawDiagnostics),
                  subtitle: Text(context.strings.redactedDeveloperEventStream),
                  children: <Widget>[
                    Container(
                      width: double.infinity,
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: Theme.of(
                          context,
                        ).colorScheme.surfaceContainerLow,
                        borderRadius: BorderRadius.circular(
                          context.torcaTokens.radiusMedium,
                        ),
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
