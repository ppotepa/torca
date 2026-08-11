import 'package:flutter/material.dart';

import 'tokens.dart';

class TorcaAvatar extends StatelessWidget {
  const TorcaAvatar({
    required this.label,
    this.size = 40,
    this.child,
    this.backgroundColor,
    this.foregroundColor,
    super.key,
  });

  final String label;
  final double size;
  final Widget? child;
  final Color? backgroundColor;
  final Color? foregroundColor;

  @override
  Widget build(BuildContext context) => Container(
    width: size,
    height: size,
    alignment: Alignment.center,
    decoration: BoxDecoration(
      color: backgroundColor ?? Theme.of(context).colorScheme.primaryContainer,
      border: Border.all(color: Theme.of(context).colorScheme.outline),
      borderRadius: BorderRadius.circular(
        context.torcaTokens.terminal ? 0 : size / 2,
      ),
    ),
    child: IconTheme(
      data: IconThemeData(color: foregroundColor),
      child: DefaultTextStyle.merge(
        style: TextStyle(color: foregroundColor),
        child:
            child ??
            Text(
              _initials(label),
              style: Theme.of(context).textTheme.labelMedium,
            ),
      ),
    ),
  );
}

String _initials(String value) {
  final words = value.trim().split(RegExp(r'\s+'));
  if (words.isEmpty || words.first.isEmpty) return '?';
  if (words.length == 1) return words.first[0].toUpperCase();
  return '${words.first[0]}${words.last[0]}'.toUpperCase();
}
