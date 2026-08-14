import 'package:flutter/material.dart';
import 'package:torca_avatar/torca_avatar.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
import 'connection_indicator.dart';
import 'radio_indicator.dart';

class ConversationSummaryTile extends StatelessWidget {
  const ConversationSummaryTile({
    required this.conversation,
    required this.contact,
    required this.selected,
    required this.onTap,
    required this.onContactInfo,
    required this.onLongPress,
    required this.onSecondaryTapDown,
    this.radio,
    this.radioSession,
    this.pinned = false,
    this.muted = false,
    super.key,
  });

  final ConversationDto conversation;
  final ContactDto? contact;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback? onContactInfo;
  final VoidCallback? onLongPress;
  final GestureTapDownCallback? onSecondaryTapDown;
  final RadioContactDto? radio;
  final RadioSessionDto? radioSession;
  final bool pinned;
  final bool muted;

  @override
  Widget build(BuildContext context) {
    final blocked = contact?.typedStatus == ContactStatus.blocked;
    final message = conversation.lastMessageBody;
    final prefix = conversation.lastMessageDirection == 'outbound'
        ? 'You: '
        : '';
    final subtitle = blocked
        ? context.strings.blocked
        : message == null || message.isEmpty
        ? context.strings.noMessagesYet
        : '$prefix$message';

    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onSecondaryTapDown: onSecondaryTapDown,
      child: ListTile(
        selected: selected,
        leading: TorcaDeviceAvatar(
          label: contact?.displayName ?? context.strings.contactLabel,
          identityId: contact?.remoteIdentityId,
          presentation: AvatarActivityPresentation.resolve(
            blocked: contact?.typedStatus == ContactStatus.blocked,
            talking:
                (radioSession?.typedState ?? radio?.typedState) ==
                RadioState.receiving,
            listening:
                (radioSession?.typedState ?? radio?.typedState) ==
                RadioState.transmitting,
            attention: conversation.unreadCount > 0,
            online: contact?.presenceState == 'online',
          ),
        ),
        title: Row(
          children: <Widget>[
            Expanded(
              child: Text(contact?.displayName ?? context.strings.contactLabel),
            ),
            if (conversation.lastActivityAtMs > 0)
              Text(
                _timeLabel(conversation.lastActivityAtMs),
                style: Theme.of(context).textTheme.labelSmall,
              ),
            if (pinned) Icon(context.torcaIcons.archive, size: 16),
            if (muted) Icon(context.torcaIcons.notifications, size: 16),
          ],
        ),
        subtitle: Row(
          children: <Widget>[
            Expanded(
              child: Text(
                subtitle,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            if (conversation.unreadCount > 0) ...<Widget>[
              const SizedBox(width: 8),
              TorcaBadge(label: Text('${conversation.unreadCount}')),
            ],
          ],
        ),
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            RadioIndicator(
              radio: radio,
              session: radioSession,
              contactName: contact?.displayName,
            ),
            ConnectionIndicator(
              state: contact?.connectionState ?? 'disconnected',
              blocked: blocked,
              showLabel: false,
            ),
            if (contact != null)
              IconButton(
                tooltip: context.strings.contactDetails,
                icon: Icon(context.torcaIcons.info, size: 19),
                onPressed: onContactInfo,
              ),
          ],
        ),
        onTap: onTap,
        onLongPress: onLongPress,
      ),
    );
  }

  static String _timeLabel(int milliseconds) {
    final value = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    final now = DateTime.now();
    if (value.year == now.year &&
        value.month == now.month &&
        value.day == now.day) {
      return '${value.hour.toString().padLeft(2, '0')}:${value.minute.toString().padLeft(2, '0')}';
    }
    if (value.year == now.year) {
      return '${value.day.toString().padLeft(2, '0')}.${value.month.toString().padLeft(2, '0')}';
    }
    return '${value.day.toString().padLeft(2, '0')}.${value.month.toString().padLeft(2, '0')}.${value.year}';
  }
}
