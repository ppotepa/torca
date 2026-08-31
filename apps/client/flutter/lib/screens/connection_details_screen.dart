import 'package:flutter/material.dart';
import 'package:torca_avatar/torca_avatar.dart';
import 'package:torca_l10n/torca_l10n.dart';
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
        return Scaffold(
          appBar: RuntimeAppBar(
            title: Text(context.l10n.connectionDetailsTitle),
          ),
          body: Center(child: Text(context.l10n.contactUnavailable)),
        );
      }
      final blocked = contact.typedStatus == ContactStatus.blocked;
      final presentation = ConnectionStatePresenter.peer(
        state: contact.peerHealth.state,
        blocked: blocked,
        icons: context.torcaIcons,
        provider: contact.transportProvider,
        strings: context.l10n,
      );
      return Scaffold(
        appBar: RuntimeAppBar(
          title: Text(context.l10n.connectionDetailsTitle),
        ),
        body: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 760),
            child: ListView(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
              children: <Widget>[
                Row(
                  children: <Widget>[
                    TorcaDeviceAvatar(
                      label: contact.displayName,
                      identityId: contact.remoteIdentityId,
                      fallbackIdentityId: contact.id,
                      size: 52,
                      presentation: AvatarActivityPresentation.resolve(
                        blocked: contact.typedStatus == ContactStatus.blocked,
                        online:
                            contact.typedAvailability ==
                            PeerAvailability.reachable,
                        waking:
                            contact.peerHealth.typedState ==
                            TransportState.connecting,
                        error:
                            contact.peerHealth.typedState ==
                            TransportState.failed,
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            contact.displayName,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: Theme.of(context).textTheme.titleLarge,
                          ),
                          const SizedBox(height: 4),
                          blocked
                              ? Text(context.l10n.blocked)
                              : PeerHealthIndicator(health: contact.peerHealth),
                        ],
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 16),
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(16),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          context.l10n.connection,
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                        const SizedBox(height: 8),
                        LayoutBuilder(
                          builder: (context, constraints) =>
                              RuntimeNetworkStatus(
                                snapshot: snapshot,
                                compact: constraints.maxWidth < 520,
                              ),
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 12),
                LayoutBuilder(
                  builder: (context, constraints) {
                    final cardWidth = constraints.maxWidth >= 560
                        ? (constraints.maxWidth - 8) / 2
                        : constraints.maxWidth;
                    return Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: <Widget>[
                        if (presentation.tone != ConnectionTone.ready)
                          SizedBox(
                            width: cardWidth,
                            child: _DetailCard(
                              label: context.l10n.status,
                              value: presentation.label,
                            ),
                          ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.transport,
                            value: presentation.label,
                          ),
                        ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.quality,
                            value: _quality(
                              contact.peerHealth.quality,
                              context,
                            ),
                          ),
                        ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.roundTrip,
                            value: contact.peerHealth.rttMs == null
                                ? context.l10n.unavailable
                                : '${contact.peerHealth.rttMs} ms',
                          ),
                        ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.lastSuccessfulProbe,
                            value: _timestamp(
                              contact.peerHealth.lastSuccessAtMs,
                              context,
                            ),
                          ),
                        ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.consecutiveFailures,
                            value: '${contact.peerHealth.consecutiveFailures}',
                          ),
                        ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.reconnectAttempts,
                            value: '${contact.peerHealth.reconnectAttempt}',
                          ),
                        ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.route,
                            value: snapshot.transport.providerRouteState,
                          ),
                        ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.peerState,
                            value: contact.peerHealth.state,
                          ),
                        ),
                        SizedBox(
                          width: cardWidth,
                          child: _DetailCard(
                            label: context.l10n.providerEndpoint,
                            value:
                                snapshot.endpointSummary ??
                                (contact.endpointAvailable
                                    ? context.l10n.providerEndpointAvailable
                                    : context
                                          .l10n
                                          .providerEndpointUnavailable),
                          ),
                        ),
                      ],
                    );
                  },
                ),
                const SizedBox(height: 16),
                Text(
                  context.l10n.connectionEvidenceNote(
                    contact.transportProvider,
                  ),
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ),
          ),
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
    'excellent' => context.l10n.excellent,
    'good' => context.l10n.good,
    'fair' => context.l10n.fair,
    'poor' => context.l10n.poor,
    _ => context.l10n.unknown,
  };

  String _timestamp(int? value, BuildContext context) {
    if (value == null || value <= 0) return context.l10n.unavailable;
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


