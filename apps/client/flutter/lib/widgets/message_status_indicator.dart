import 'package:flutter/material.dart';

class MessageStatusIndicator extends StatelessWidget {
  const MessageStatusIndicator({required this.status, super.key});

  final String status;

  @override
  Widget build(BuildContext context) {
    final presentation = _presentation(status);
    return Tooltip(
      message: presentation.label,
      child: Semantics(
        label: presentation.label,
        child: Icon(
          presentation.icon,
          size: 14,
          color: presentation.isError
              ? Theme.of(context).colorScheme.error
              : Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      ),
    );
  }
}

({IconData icon, String label, bool isError}) _presentation(String status) =>
    switch (status) {
      'queued' => (icon: Icons.schedule, label: 'Queued', isError: false),
      'sending' => (icon: Icons.sync, label: 'Sending', isError: false),
      'sent' => (icon: Icons.check, label: 'Sent', isError: false),
      'delivered' => (icon: Icons.done_all, label: 'Delivered', isError: false),
      'read' => (icon: Icons.done_all, label: 'Read', isError: false),
      'failed' => (
        icon: Icons.error_outline,
        label: 'Delivery failed',
        isError: true,
      ),
      'cancelled' => (
        icon: Icons.cancel_outlined,
        label: 'Cancelled',
        isError: true,
      ),
      _ => (icon: Icons.info_outline, label: status, isError: false),
    };
