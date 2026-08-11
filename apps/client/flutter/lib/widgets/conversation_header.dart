import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';

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
        TorcaAvatar(label: name, size: compact ? 32 : 40),
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
                    : InkWell(
                        onTap: onConnectionDetails,
                        child: Text(
                          _presenceLabel(context, value),
                          style: Theme.of(context).textTheme.bodySmall
                              ?.copyWith(
                                color: value.presenceState == 'online'
                                    ? Theme.of(context).colorScheme.primary
                                    : null,
                              ),
                        ),
                      ),
            ],
          ),
        ),
        if (!compact) ...<Widget>[
          const Spacer(),
          IconButton(
            tooltip: 'Connection details',
            onPressed: onConnectionDetails,
            icon: Icon(context.torcaIcons.info),
          ),
        ],
      ],
    );
  }
}

String _presenceLabel(BuildContext context, ContactDto contact) {
  if (contact.presenceState == 'online') return 'online';
  final milliseconds = contact.lastSeenAtMs;
  if (milliseconds == null || milliseconds <= 0) {
    return contact.connectionState == 'reconnecting'
        ? 'reconnecting'
        : 'offline';
  }
  final value = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
  final now = DateTime.now();
  final time = MaterialLocalizations.of(
    context,
  ).formatTimeOfDay(TimeOfDay.fromDateTime(value));
  if (value.year == now.year &&
      value.month == now.month &&
      value.day == now.day) {
    return 'last seen today at $time';
  }
  final yesterday = now.subtract(const Duration(days: 1));
  if (value.year == yesterday.year &&
      value.month == yesterday.month &&
      value.day == yesterday.day) {
    return 'last seen yesterday at $time';
  }
  return 'last seen ${MaterialLocalizations.of(context).formatMediumDate(value)}';
}
