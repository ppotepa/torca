import 'package:flutter/material.dart';

import '../generated/torca_contract.dart';
import 'peer_health_indicator.dart';

class ConversationHeader extends StatelessWidget {
  const ConversationHeader({
    required this.contact,
    required this.onConnectionDetails,
    this.compact = false,
    super.key,
  });

  final ContactDto? contact;
  final VoidCallback onConnectionDetails;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final value = contact;
    final blocked = value?.status == 'blocked';
    final name = value?.displayName ?? 'Contact';
    return Row(
      mainAxisSize: compact ? MainAxisSize.min : MainAxisSize.max,
      children: <Widget>[
        CircleAvatar(
          radius: compact ? 16 : 20,
          child: const Icon(Icons.person_outline),
        ),
        const SizedBox(width: 10),
        Flexible(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: compact
                    ? Theme.of(context).textTheme.titleSmall
                    : Theme.of(context).textTheme.titleMedium,
              ),
              if (value != null)
                blocked
                    ? Text(
                        'Blocked',
                        style: Theme.of(context).textTheme.bodySmall,
                      )
                    : PeerHealthIndicator(
                        health: value.peerHealth,
                        onPressed: onConnectionDetails,
                      ),
            ],
          ),
        ),
        if (!compact) ...<Widget>[
          const Spacer(),
          IconButton(
            tooltip: 'Connection details',
            onPressed: onConnectionDetails,
            icon: const Icon(Icons.info_outline),
          ),
        ],
      ],
    );
  }
}
