import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
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
        return Scaffold(
          appBar: RuntimeAppBar(
            title: Text(context.strings.connectionDetailsTitle),
          ),
          body: Center(child: Text(context.strings.contactUnavailable)),
        );
      }
      final blocked = contact.typedStatus == ContactStatus.blocked;
      final presentation = ConnectionStatePresenter.peer(
        state: contact.peerHealth.state,
        blocked: blocked,
        icons: context.torcaIcons,
        strings: context.strings,
      );
      return Scaffold(
        appBar: RuntimeAppBar(
          title: Text(context.strings.connectionDetailsTitle),
        ),
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
                          ? Text(context.strings.blocked)
                          : PeerHealthIndicator(health: contact.peerHealth),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 24),
            if (presentation.label !=
                context.strings.directP2pOverTor) ...<Widget>[
              _DetailCard(
                label: context.strings.status,
                value: presentation.label,
              ),
              const SizedBox(height: 8),
            ],
            _DetailCard(
              label: context.strings.transport,
              value: context.strings.directP2pOverTor,
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: 'Quality',
              value: _quality(contact.peerHealth.quality, context),
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: context.strings.roundTrip,
              value: contact.peerHealth.rttMs == null
                  ? context.strings.unavailable
                  : '${contact.peerHealth.rttMs} ms',
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: context.strings.lastSuccessfulProbe,
              value: _timestamp(contact.peerHealth.lastSuccessAtMs, context),
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: context.strings.consecutiveFailures,
              value: '${contact.peerHealth.consecutiveFailures}',
            ),
            const SizedBox(height: 8),
            _DetailCard(
              label: context.strings.reconnectAttempts,
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

  String _quality(String value, BuildContext context) => switch (value) {
    'excellent' => context.strings.excellent,
    'good' => context.strings.good,
    'fair' => context.strings.fair,
    'poor' => context.strings.poor,
    _ => context.strings.unknown,
  };

  String _timestamp(int? value, BuildContext context) {
    if (value == null || value <= 0) return context.strings.unavailable;
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
