import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';

class ConversationHeader extends StatelessWidget {
  const ConversationHeader({
    required this.contact,
    required this.onConnectionDetails,
    this.gateway,
    this.radio,
    this.session,
    this.compact = false,
    super.key,
  });

  final ContactDto? contact;
  final VoidCallback onConnectionDetails;
  final bool compact;
  final EngineGateway? gateway;
  final RadioContactDto? radio;
  final RadioSessionDto? session;

  @override
  Widget build(BuildContext context) {
    final value = contact;
    final blocked = value?.typedStatus == ContactStatus.blocked;
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
                                color:
                                    value.typedPresenceState ==
                                        PresenceState.online
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
          if (value != null && gateway != null && radio != null)
            _RadioHeaderAction(
              gateway: gateway!,
              contact: value,
              radio: radio!,
              session: session,
              compact: compact,
            ),
          IconButton(
            tooltip: context.strings.connectionDetails,
            onPressed: onConnectionDetails,
            icon: Icon(context.torcaIcons.info),
          ),
        ],
        if (compact && value != null && gateway != null && radio != null)
          _RadioHeaderAction(
            gateway: gateway!,
            contact: value,
            radio: radio!,
            session: session,
            compact: compact,
          ),
      ],
    );
  }
}

class _RadioHeaderAction extends StatelessWidget {
  const _RadioHeaderAction({
    required this.gateway,
    required this.contact,
    required this.radio,
    this.session,
    required this.compact,
  });

  final EngineGateway gateway;
  final ContactDto contact;
  final RadioContactDto radio;
  final RadioSessionDto? session;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final state = session?.typedState ?? radio.typedState;
    final capturing = state == RadioState.transmitting;
    final receiving = state == RadioState.receiving;
    final colors = Theme.of(context).colorScheme;
    return Semantics(
      label: capturing
          ? context.strings.radioTransmitting
          : receiving
          ? context.strings.radioReceiving(contact.displayName)
          : context.strings.radioMode,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (capturing)
            Padding(
              padding: const EdgeInsets.only(right: 4),
              child: Icon(
                Icons.fiber_manual_record,
                size: 12,
                color: colors.error,
              ),
            ),
          if (!compact)
            Text(
              capturing
                  ? 'REC'
                  : receiving
                  ? 'RX'
                  : context.strings.radioMode,
            ),
          Switch.adaptive(
            value: radio.localEnabled,
            onChanged: (enabled) => _setEnabled(context, enabled),
          ),
        ],
      ),
    );
  }

  Future<void> _setEnabled(BuildContext context, bool enabled) async {
    final result = await gateway.execute(
      SetRadioEnabledCommandDto(contactIdHex: contact.id, enabled: enabled),
    );
    if (!context.mounted || result.ok) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(context.strings.couldNotStartRadio)));
  }
}

String _presenceLabel(BuildContext context, ContactDto contact) {
  if (contact.typedPresenceState == PresenceState.online) return 'online';
  final milliseconds = contact.lastSeenAtMs;
  if (milliseconds == null || milliseconds <= 0) {
    return contact.peerHealth.typedState == TransportState.reconnecting
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
