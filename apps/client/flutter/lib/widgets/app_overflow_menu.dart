import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

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
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.addContact),
          title: const Text('New pairing'),
        ),
      ),
      PopupMenuItem(
        value: AppOverflowAction.identity,
        enabled: hasIdentity,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.identity),
          title: const Text('Your identity'),
        ),
      ),
      const PopupMenuDivider(),
      PopupMenuItem(
        value: AppOverflowAction.diagnostics,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.diagnostics),
          title: const Text('Diagnostics'),
        ),
      ),
      PopupMenuItem(
        value: AppOverflowAction.settings,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.settings),
          title: const Text('Settings'),
        ),
      ),
      PopupMenuItem(
        value: AppOverflowAction.about,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.info),
          title: const Text('About Torca'),
        ),
      ),
    ],
  );
}
