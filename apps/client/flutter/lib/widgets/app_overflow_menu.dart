import 'package:flutter/material.dart';

enum AppOverflowAction { pairing, identity, diagnostics, settings, about }

class AppOverflowMenu extends StatelessWidget {
  const AppOverflowMenu({
    required this.hasIdentity,
    required this.onSelected,
    super.key,
  });

  final bool hasIdentity;
  final ValueChanged<AppOverflowAction> onSelected;

  @override
  Widget build(BuildContext context) => PopupMenuButton<AppOverflowAction>(
    tooltip: 'Application menu',
    onSelected: onSelected,
    itemBuilder: (context) => <PopupMenuEntry<AppOverflowAction>>[
      PopupMenuItem(
        value: AppOverflowAction.pairing,
        enabled: hasIdentity,
        child: const ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(Icons.person_add_alt_1),
          title: Text('New pairing'),
        ),
      ),
      PopupMenuItem(
        value: AppOverflowAction.identity,
        enabled: hasIdentity,
        child: const ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(Icons.shield_outlined),
          title: Text('Your identity'),
        ),
      ),
      const PopupMenuDivider(),
      const PopupMenuItem(
        value: AppOverflowAction.diagnostics,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(Icons.monitor_heart_outlined),
          title: Text('Diagnostics'),
        ),
      ),
      const PopupMenuItem(
        value: AppOverflowAction.settings,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(Icons.settings_outlined),
          title: Text('Settings'),
        ),
      ),
      const PopupMenuItem(
        value: AppOverflowAction.about,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(Icons.info_outline),
          title: Text('About Torca'),
        ),
      ),
    ],
  );
}
