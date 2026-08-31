import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';

class MessageStatusIndicator extends StatelessWidget {
  const MessageStatusIndicator({required this.status, super.key});

  final MessageStatus status;

  @override
  Widget build(BuildContext context) {
    final presentation = _presentation(context, status);
    return Tooltip(
      message: presentation.label,
      child: Semantics(
        label: presentation.label,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(
              presentation.icon,
              size: 14,
              color: presentation.isError
                  ? Theme.of(context).colorScheme.error
                  : status == MessageStatus.read
                  ? Theme.of(context).colorScheme.primary
                  : Theme.of(context).colorScheme.onSurfaceVariant,
            ),
            if (status == MessageStatus.queued ||
                status == MessageStatus.sending ||
                status == MessageStatus.failed ||
                status == MessageStatus.cancelled) ...<Widget>[
              const SizedBox(width: 3),
              Text(
                presentation.label,
                style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: presentation.isError
                      ? Theme.of(context).colorScheme.error
                      : Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

({IconData icon, String label, bool isError}) _presentation(
  BuildContext context,
  MessageStatus status,
) => switch (status) {
  MessageStatus.queued => (
    icon: context.torcaIcons.queued,
    label: context.strings.queued,
    isError: false,
  ),
  MessageStatus.sending => (
    icon: context.torcaIcons.sending,
    label: context.strings.sendingSecurely,
    isError: false,
  ),
  MessageStatus.sent => (
    icon: context.torcaIcons.sent,
    label: context.strings.sent,
    isError: false,
  ),
  MessageStatus.delivered => (
    icon: context.torcaIcons.delivered,
    label: context.strings.delivered,
    isError: false,
  ),
  MessageStatus.read => (
    icon: context.torcaIcons.read,
    label: context.strings.read,
    isError: false,
  ),
  MessageStatus.failed => (
    icon: context.torcaIcons.error,
    label: context.strings.deliveryFailed,
    isError: true,
  ),
  MessageStatus.cancelled => (
    icon: context.torcaIcons.cancelled,
    label: context.strings.cancelled,
    isError: true,
  ),
  MessageStatus.deleted => (
    icon: context.torcaIcons.remove,
    label: context.strings.messageDeleted,
    isError: false,
  ),
  MessageStatus.unknown => (
    icon: context.torcaIcons.info,
    label: context.strings.unavailable,
    isError: false,
  ),
};
