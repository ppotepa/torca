import 'dart:convert';
import 'dart:typed_data';

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
  String? _logTailsJson;
  String? _error;
  bool _loading = false;
  bool _loadingLogTails = false;

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

  Future<void> _refreshLogTails() async {
    setState(() {
      _loadingLogTails = true;
      _error = null;
    });
    try {
      final value = await widget.gateway.diagnosticsLogTailsJson();
      if (mounted) setState(() => _logTailsJson = value);
    } on Object catch (error) {
      if (mounted) setState(() => _error = '$error');
    } finally {
      if (mounted) setState(() => _loadingLogTails = false);
    }
  }

  Future<void> _export() async {
    final value = _json ?? await widget.gateway.diagnosticsJson();
    final path = await FilePicker.saveFile(
      dialogTitle: 'Export Torca diagnostics',
      fileName: 'torca-diagnostics.json',
      bytes: Uint8List.fromList(utf8.encode(value)),
      mimeType: 'application/json',
    );
    if (path == null || !mounted) return;
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(context.strings.diagnosticsExported)),
      );
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

  Future<void> _observation(BridgeCommandDto command) async {
    setState(() => _loading = true);
    try {
      final result = await widget.gateway.execute(command);
      if (!result.ok)
        throw StateError(result.errorCode ?? 'Observation command failed');
      await _refresh();
    } on Object catch (error) {
      if (mounted) setState(() => _error = '$error');
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _markIncident() async {
    await _observation(const MarkIncidentCommandDto());
    if (!mounted || _error != null) return;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text(
          'Incident snapshot saved to this run\'s local diagnostics.',
        ),
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
  Widget build(BuildContext context) => DefaultTabController(
    length: 4,
    child: Scaffold(
      appBar: RuntimeAppBar(
        title: const Text('Debug'),
        actions: <Widget>[
          IconButton(
            tooltip: context.strings.refresh,
            onPressed: _loading ? null : _refresh,
            icon: Icon(context.torcaIcons.retry),
          ),
        ],
        bottom: const TabBar(
          isScrollable: true,
          tabs: <Widget>[
            Tab(text: 'Battery'),
            Tab(text: 'Runtime'),
            Tab(text: 'Logs'),
            Tab(text: 'Incident'),
          ],
        ),
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
              builder: (context, snapshot, _) => TabBarView(
                children: <Widget>[
                  _batteryTab(context),
                  _runtimeTab(context, snapshot),
                  _logsTab(context),
                  _incidentTab(context),
                ],
              ),
            ),
    ),
  );

  Widget _batteryTab(BuildContext context) => ListView(
    padding: const EdgeInsets.all(16),
    children: <Widget>[
      _ObservationCard(
        data: _observationData(),
        busy: _loading,
        onStart: () => _observation(const StartBatteryObservationCommandDto()),
        onStop: () => _observation(const StopBatteryObservationCommandDto()),
        onReset: () => _observation(const ResetBatteryObservationCommandDto()),
      ),
      const SizedBox(height: 12),
      _WhyAwakeCard(data: _whyAwake()),
    ],
  );

  Widget _runtimeTab(BuildContext context, AppSnapshotDto snapshot) => ListView(
    padding: const EdgeInsets.all(16),
    children: <Widget>[
      Text('Runtime health', style: Theme.of(context).textTheme.titleLarge),
      const SizedBox(height: 12),
      DiagnosticsOverview(
        snapshot: snapshot,
        diagnosticsReadable: _hasReadableEvents(_json),
      ),
      const SizedBox(height: 12),
      _WhyAwakeCard(data: _whyAwake()),
    ],
  );

  Widget _logsTab(BuildContext context) => ListView(
    padding: const EdgeInsets.all(16),
    children: <Widget>[
      Text('Native log tails', style: Theme.of(context).textTheme.titleLarge),
      const SizedBox(height: 6),
      const Text(
        'Loads a bounded, redacted tail from current-run native logs only. '
        'This is an explicit diagnostic read and does not poll or keep a watcher alive.',
      ),
      const SizedBox(height: 12),
      Align(
        alignment: Alignment.centerLeft,
        child: OutlinedButton.icon(
          onPressed: _loadingLogTails ? null : _refreshLogTails,
          icon: _loadingLogTails
              ? const SizedBox.square(
                  dimension: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Icon(context.torcaIcons.retry),
          label: const Text('Load current run logs'),
        ),
      ),
      const SizedBox(height: 12),
      Container(
        width: double.infinity,
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.surfaceContainerLow,
          borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
        ),
        child: SelectableText(
          _pretty(_logTailsJson ?? '{"logs":[],"hint":"Not loaded"}'),
        ),
      ),
    ],
  );

  Widget _incidentTab(BuildContext context) => ListView(
    padding: const EdgeInsets.all(16),
    children: <Widget>[
      Text('Incident', style: Theme.of(context).textTheme.titleLarge),
      const SizedBox(height: 6),
      const Text(
        'Run a self-test, then export the redacted snapshot when marking an incident. '
        'Message text, attachments, audio and secrets are not included.',
      ),
      const SizedBox(height: 16),
      Wrap(
        spacing: 8,
        runSpacing: 8,
        children: <Widget>[
          FilledButton.tonalIcon(
            onPressed: _loading ? null : _selfTest,
            icon: Icon(context.torcaIcons.diagnostics),
            label: Text(context.strings.runSelfTest),
          ),
          FilledButton.icon(
            onPressed: _loading ? null : _markIncident,
            icon: Icon(context.torcaIcons.diagnostics),
            label: const Text('Mark incident'),
          ),
          OutlinedButton.icon(
            onPressed: _loading ? null : _export,
            icon: Icon(context.torcaIcons.save),
            label: Text(context.strings.exportDiagnostics),
          ),
        ],
      ),
    ],
  );

  String _pretty(String value) {
    try {
      return const JsonEncoder.withIndent('  ').convert(jsonDecode(value));
    } on FormatException {
      return value;
    }
  }

  Map<String, dynamic>? _whyAwake() {
    final raw = _json;
    if (raw == null) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return null;
      final value = decoded['whyAwake'];
      return value is Map<String, dynamic> ? value : null;
    } on FormatException {
      return null;
    }
  }

  Map<String, dynamic>? _observationData() {
    final raw = _json;
    if (raw == null) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return null;
      final value = decoded['observation'];
      return value is Map<String, dynamic> ? value : null;
    } on FormatException {
      return null;
    }
  }
}

class _ObservationCard extends StatelessWidget {
  const _ObservationCard({
    required this.data,
    required this.busy,
    required this.onStart,
    required this.onStop,
    required this.onReset,
  });

  final Map<String, dynamic>? data;
  final bool busy;
  final VoidCallback onStart, onStop, onReset;

  @override
  Widget build(BuildContext context) {
    final active = data?['active'] == true;
    final wakeSources = data?['wakeSources'] is Map
        ? (data!['wakeSources'] as Map).entries
              .map((entry) => '${entry.key}: ${entry.value}')
              .join(', ')
        : 'none';
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'Battery observation',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 6),
            Text(
              active
                  ? 'Recording deltas since the observation baseline.'
                  : 'Start before an idle or recovery scenario to record only new work.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 12),
            Text('State: ${active ? 'recording' : 'stopped'}'),
            Text('Work: ${data?['totalWork'] ?? 0}'),
            Text('Regression score: ${data?['energyScore'] ?? 0}'),
            Text('Wake sources: $wakeSources'),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                FilledButton.tonal(
                  onPressed: busy || active ? null : onStart,
                  child: const Text('Start observation'),
                ),
                OutlinedButton(
                  onPressed: busy || !active ? null : onStop,
                  child: const Text('Stop observation'),
                ),
                OutlinedButton(
                  onPressed: busy ? null : onReset,
                  child: const Text('Reset baseline'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _WhyAwakeCard extends StatelessWidget {
  const _WhyAwakeCard({required this.data});

  final Map<String, dynamic>? data;

  @override
  Widget build(BuildContext context) {
    final value = data;
    if (value == null) return const SizedBox.shrink();
    final reasons = value['leaseReasons'] is Map
        ? (value['leaseReasons'] as Map).entries
              .map((entry) => '${entry.key}: ${entry.value}')
              .join(', ')
        : 'none';
    final scheduled = value['scheduledWork'] is Map
        ? (value['scheduledWork'] as Map).entries
              .map((entry) => '${entry.key}: ${entry.value}')
              .join(', ')
        : 'none';
    final deadline = value['nextDeadlineInMs'];
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text('Why awake', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 6),
            Text(
              'Redacted scheduler explanation; contact identifiers are never shown here.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 12),
            _row('Active leases', '${value['activeLeases'] ?? 0}'),
            _row('Active demands', '${value['activeDemands'] ?? 0}'),
            _row('Lease reasons', reasons),
            _row('Scheduled work', scheduled),
            _row('Next deadline', deadline == null ? 'none' : '${deadline} ms'),
          ],
        ),
      ),
    );
  }

  Widget _row(String label, String value) => Padding(
    padding: const EdgeInsets.only(top: 4),
    child: Text('$label: $value'),
  );
}

class _Check {
  const _Check(this.name, this.ok, this.detail);
  final String name;
  final bool ok;
  final String detail;
}
