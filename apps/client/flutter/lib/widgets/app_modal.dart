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
    child: SizedBox(
      width: 460,
      height: height,
      child: Column(
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 20, 12, 8),
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
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
              child: child,
            ),
          ),
        ],
      ),
    ),
  );
}
