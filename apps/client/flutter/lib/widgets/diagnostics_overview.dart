import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';

class DiagnosticsOverview extends StatelessWidget {
  const DiagnosticsOverview({
    required this.snapshot,
    required this.diagnosticsReadable,
    super.key,
  });

  final AppSnapshotDto snapshot;
  final bool diagnosticsReadable;

  @override
  Widget build(BuildContext context) {
    final readyPeers = snapshot.contacts
        .where((contact) => contact.peerHealth.state == 'ready')
        .length;
    final totalPeers = snapshot.contacts.length;
    final peerDetail = totalPeers == 0
        ? 'No contacts paired'
        : '$readyPeers of $totalPeers direct peer links ready';

    final checks = <_OverviewItem>[
      _OverviewItem(
        'Native bridge',
        true,
        'Contract $torcaContractVersion snapshot readable',
        context.torcaIcons.diagnostics,
      ),
      _OverviewItem(
        'Local identity',
        snapshot.identity != null,
        snapshot.identity == null ? 'Not initialized' : 'Loaded',
        context.torcaIcons.identity,
      ),
      _OverviewItem(
        'Tor',
        snapshot.torState == 'ready',
        'State: ${snapshot.torState}',
        context.torcaIcons.identity,
      ),
      _OverviewItem(
        'Onion service',
        (snapshot.onionAddress ?? '').endsWith('.onion'),
        snapshot.onionAddress == null ? 'No onion address' : 'Published',
        context.torcaIcons.link,
      ),
      _OverviewItem(
        'Direct peers',
        totalPeers == 0 || readyPeers > 0,
        peerDetail,
        context.torcaIcons.online,
      ),
      _OverviewItem(
        'Diagnostics stream',
        diagnosticsReadable,
        diagnosticsReadable
            ? 'Redacted health events readable'
            : 'No readable health events',
        context.torcaIcons.diagnostics,
      ),
    ];

    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth >= 720
            ? 3
            : constraints.maxWidth >= 440
            ? 2
            : 1;
        final width = (constraints.maxWidth - (columns - 1) * 10) / columns;
        return Wrap(
          spacing: 10,
          runSpacing: 10,
          children: checks
              .map(
                (item) => SizedBox(
                  width: width,
                  child: _HealthCard(item: item),
                ),
              )
              .toList(growable: false),
        );
      },
    );
  }
}

class _HealthCard extends StatelessWidget {
  const _HealthCard({required this.item});
  final _OverviewItem item;

  @override
  Widget build(BuildContext context) => Card(
    child: Padding(
      padding: const EdgeInsets.all(12),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Icon(
            item.ok ? context.torcaIcons.success : context.torcaIcons.error,
            color: item.ok
                ? Theme.of(context).colorScheme.primary
                : Theme.of(context).colorScheme.error,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Icon(item.icon, size: 16),
                    const SizedBox(width: 5),
                    Expanded(child: Text(item.name)),
                  ],
                ),
                const SizedBox(height: 4),
                Text(item.detail, style: Theme.of(context).textTheme.bodySmall),
              ],
            ),
          ),
        ],
      ),
    ),
  );
}

class _OverviewItem {
  const _OverviewItem(this.name, this.ok, this.detail, this.icon);
  final String name;
  final bool ok;
  final String detail;
  final IconData icon;
}
