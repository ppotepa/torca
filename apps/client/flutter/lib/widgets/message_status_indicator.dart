import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

class MessageStatusIndicator extends StatelessWidget {
  const MessageStatusIndicator({required this.status, super.key});

  final String status;

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
              : status == 'read'
              ? Theme.of(context).colorScheme.primary
              : Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      ),
    );
  }
}

({IconData icon, String label, bool isError}) _presentation(
  BuildContext context,
  String status,
) => switch (status) {
  'queued' => (
    icon: context.torcaIcons.queued,
    label: 'Queued',
    isError: false,
  ),
  'sending' => (
    icon: context.torcaIcons.sending,
    label: 'Sending',
    isError: false,
  ),
  'sent' => (icon: context.torcaIcons.sent, label: 'Sent', isError: false),
  'delivered' => (
    icon: context.torcaIcons.delivered,
    label: 'Delivered',
    isError: false,
  ),
  'read' => (icon: context.torcaIcons.read, label: 'Read', isError: false),
  'failed' => (
    icon: context.torcaIcons.error,
    label: 'Delivery failed',
    isError: true,
  ),
  'cancelled' => (
    icon: context.torcaIcons.cancelled,
    label: 'Cancelled',
    isError: true,
  ),
  _ => (icon: context.torcaIcons.info, label: status, isError: false),
};
