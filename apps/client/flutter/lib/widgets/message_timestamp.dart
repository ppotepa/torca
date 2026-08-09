import 'package:flutter/material.dart';

class MessageTimestamp extends StatelessWidget {
  const MessageTimestamp({required this.milliseconds, super.key});

  final int milliseconds;

  @override
  Widget build(BuildContext context) {
    if (milliseconds <= 0) return const SizedBox.shrink();
    final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    final localizations = MaterialLocalizations.of(context);
    return Tooltip(
      message:
          '${localizations.formatMediumDate(date)} ${localizations.formatTimeOfDay(TimeOfDay.fromDateTime(date))}',
      child: Text(
        localizations.formatTimeOfDay(TimeOfDay.fromDateTime(date)),
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
          color: Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      ),
    );
  }
}
