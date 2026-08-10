import 'package:flutter/material.dart';

/// Consistent responsive container for details and confirmation flows.
class AppModal extends StatelessWidget {
  const AppModal({
    required this.title,
    required this.child,
    this.height = 650,
    super.key,
  });

  final String title;
  final Widget child;
  final double height;

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
        height: height,
        child: Material(
          borderRadius: BorderRadius.circular(24),
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
                      icon: const Icon(Icons.close),
                    ),
                  ],
                ),
              ),
              Divider(
                height: 1,
                color: Theme.of(context).colorScheme.outlineVariant,
              ),
              Expanded(
                child: SingleChildScrollView(
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
