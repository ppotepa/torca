import 'package:flutter/material.dart';

/// A stable-size action button that makes asynchronous work explicit.
class AsyncActionButton extends StatelessWidget {
  const AsyncActionButton({
    required this.label,
    required this.onPressed,
    this.busy = false,
    this.icon,
    super.key,
  });

  final String label;
  final VoidCallback? onPressed;
  final bool busy;
  final IconData? icon;

  @override
  Widget build(BuildContext context) => FilledButton.icon(
    onPressed: busy ? null : onPressed,
    icon: busy
        ? const SizedBox(
            width: 18,
            height: 18,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        : Icon(icon ?? Icons.arrow_forward),
    label: Text(busy ? '$label…' : label),
  );
}
