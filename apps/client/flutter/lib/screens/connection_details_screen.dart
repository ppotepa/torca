import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/connection_state_presenter.dart';
import '../widgets/peer_health_indicator.dart';
import '../widgets/runtime_network_status.dart';

class ConnectionDetailsScreen extends StatelessWidget {
  const ConnectionDetailsScreen({
    required this.gateway,
    required this.contactId,
    super.key,
  });

  final EngineGateway gateway;
  final String contactId;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: gateway.snapshots,
    builder: (context, snapshot, _) {
      final contact = _contact(snapshot);
      if (contact == null) {
        return const Scaffold(
          appBar: const RuntimeAppBar(title: Text('Connection details')),
          body: const Center(
            child: Text('This contact is no longer available.'),
          ),
        );
      }
      final blocked = contact.status == 'blocked';
      final presentation = ConnectionStatePresenter.peer(
        state: contact.peerHealth.state,
        blocked: blocked,
        icons: context.torcaIcons,
      );
      return Scaffold(
        appBar: const RuntimeAppBar(title: Text('Connection details')),
        body: ListView(
          padding: const EdgeInsets.all(20),
          children: <Widget>[
            Row(
              children: <Widget>[
                TorcaAvatar(
                  label: contact.displayName,
                  size: 52,
                  child: Icon(context.torcaIcons.contacts),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        contact.displayName,
                        style: Theme.of(context).textTheme.titleLarge,
                      ),
                      const SizedBox(height: 4),
                      blocked
                          ? const Text('Blocked')
                          : PeerHealthIndicator(health: contact.peerHealth),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 24),
            if (presentation.label != 'Direct P2P over Tor') ...<Widget>[
              _DetailCard(label: 'Status', value: presentation.label),
              const SizedBox(height: 8),
            ],
            const _DetailCard(label: 'Transport', value: 'Direct P2P over Tor'),
            const SizedBox(height: 8),
            _DetailCard(
              label: 'Quality',
              value: _quality(contact.peerHealth.quality),
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: 'Round trip',
              value: contact.peerHealth.rttMs == null
                  ? 'Unavailable'
                  : '${contact.peerHealth.rttMs} ms',
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: 'Last successful probe',
              value: _timestamp(contact.peerHealth.lastSuccessAtMs),
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: 'Consecutive failures',
              value: '${contact.peerHealth.consecutiveFailures}',
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: 'Reconnect attempts',
              value: '${contact.peerHealth.reconnectAttempt}',
            ),
            const SizedBox(height: 20),
            Text(
              'Quality describes the authenticated direct peer link through Tor. It is based on runtime probe history and reconnect health, not radio or internet signal strength.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      );
    },
  );

  ContactDto? _contact(AppSnapshotDto snapshot) {
    for (final contact in snapshot.contacts) {
      if (contact.id == contactId) return contact;
    }
    return null;
  }

  static String _quality(String value) => switch (value) {
    'excellent' => 'Excellent',
    'good' => 'Good',
    'fair' => 'Fair',
    'poor' => 'Poor',
    _ => 'Unknown',
  };

  static String _timestamp(int? value) {
    if (value == null || value <= 0) return 'Unavailable';
    return DateTime.fromMillisecondsSinceEpoch(value).toLocal().toString();
  }
}

class _DetailCard extends StatelessWidget {
  const _DetailCard({required this.label, required this.value});
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) => Card(
    child: ListTile(title: Text(label), subtitle: SelectableText(value)),
  );
}
