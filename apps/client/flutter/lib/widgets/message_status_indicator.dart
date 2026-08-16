import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';

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
        child: Icon(
          presentation.icon,
          size: 14,
          color: presentation.isError
              ? Theme.of(context).colorScheme.error
              : status == MessageStatus.read
              ? Theme.of(context).colorScheme.primary
              : Theme.of(context).colorScheme.onSurfaceVariant,
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
    label: 'Queued',
    isError: false,
  ),
  MessageStatus.sending => (
    icon: context.torcaIcons.sending,
    label: 'Sending',
    isError: false,
  ),
  MessageStatus.sent => (icon: context.torcaIcons.sent, label: 'Sent', isError: false),
  MessageStatus.delivered => (
    icon: context.torcaIcons.delivered,
    label: 'Delivered',
    isError: false,
  ),
  MessageStatus.read => (icon: context.torcaIcons.read, label: 'Read', isError: false),
  MessageStatus.failed => (
    icon: context.torcaIcons.error,
    label: 'Delivery failed',
    isError: true,
  ),
  MessageStatus.cancelled => (
    icon: context.torcaIcons.cancelled,
    label: 'Cancelled',
    isError: true,
  ),
  MessageStatus.unknown => (
    icon: context.torcaIcons.info,
    label: 'Unknown',
    isError: false,
  ),
};
