import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

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
  Widget build(BuildContext context) => Semantics(
    button: true,
    enabled: !busy && onPressed != null,
    label: label,
    value: busy ? 'In progress' : null,
    child: FilledButton.icon(
      onPressed: busy ? null : onPressed,
      // Keep both the label and the icon slot identical while the operation
      // runs. Changing `label` to `label…` used to resize parent rows and made
      // the whole modal/panel appear to jump on press.
      icon: SizedBox(
        width: 18,
        height: 18,
        child: Center(
          child: busy
              ? const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Icon(icon ?? context.torcaIcons.send, size: 18),
        ),
      ),
      label: Text(label),
    ),
  );
}
