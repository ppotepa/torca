import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

/// Consistent responsive container for details and confirmation flows.
class AppModal extends StatelessWidget {
  const AppModal({
    required this.title,
    required this.child,
    this.height = 650,
    this.scrollable = true,
    super.key,
  });

  final String title;
  final Widget child;
  final double height;
  final bool scrollable;

  @override
  Widget build(BuildContext context) => Dialog(
    insetPadding: const EdgeInsets.symmetric(horizontal: 18, vertical: 24),
    child: ConstrainedBox(
      constraints: BoxConstraints(
        maxWidth: 520,
        maxHeight: MediaQuery.sizeOf(context).height - 48,
      ),
      child: SizedBox(
        width: 520,
        height: height
            .clamp(280.0, MediaQuery.sizeOf(context).height - 48)
            .toDouble(),
        child: Material(
          borderRadius: BorderRadius.circular(context.torcaTokens.radiusLarge),
          clipBehavior: Clip.antiAlias,
          child: Column(
            children: <Widget>[
              Padding(
                padding: const EdgeInsets.fromLTRB(24, 18, 12, 12),
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        title,
                        style: Theme.of(context).textTheme.titleLarge,
                      ),
                    ),
                    IconButton(
                      tooltip: 'Close',
                      onPressed: () => Navigator.of(context).pop(),
                      icon: Icon(context.torcaIcons.close),
                    ),
                  ],
                ),
              ),
              Divider(
                height: 1,
                color: Theme.of(context).colorScheme.outlineVariant,
              ),
              Expanded(
                child: scrollable
                    ? SingleChildScrollView(
                        padding: const EdgeInsets.fromLTRB(24, 18, 24, 24),
                        child: child,
                      )
                    : Padding(
                        padding: const EdgeInsets.fromLTRB(24, 18, 24, 24),
                        child: child,
                      ),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}
