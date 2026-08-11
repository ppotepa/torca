import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

class ReplyQuote extends StatelessWidget {
  const ReplyQuote({required this.body, required this.unavailable, super.key});

  final String body;
  final bool unavailable;

  @override
  Widget build(BuildContext context) => Container(
    width: double.infinity,
    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.55),
      borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
    ),
    child: Text(
      body,
      maxLines: 2,
      overflow: TextOverflow.ellipsis,
      style: unavailable ? Theme.of(context).textTheme.bodySmall : null,
    ),
  );
}
