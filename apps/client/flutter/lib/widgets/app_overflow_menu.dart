import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
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
    tooltip: context.l10n.applicationMenu,
    onSelected: onSelected,
    itemBuilder: (context) => <PopupMenuEntry<AppOverflowAction>>[
      PopupMenuItem(
        value: AppOverflowAction.pairing,
        enabled: hasIdentity,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.addContact),
          title: Text(context.l10n.newPairing),
        ),
      ),
      PopupMenuItem(
        value: AppOverflowAction.identity,
        enabled: hasIdentity,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.identity),
          title: Text(context.l10n.yourIdentity),
        ),
      ),
      if (kDebugMode) ...<PopupMenuEntry<AppOverflowAction>>[
        const PopupMenuDivider(),
        PopupMenuItem(
          value: AppOverflowAction.diagnostics,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.diagnostics),
            title: Text(context.l10n.diagnostics),
          ),
        ),
      ],
      PopupMenuItem(
        value: AppOverflowAction.settings,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.settings),
          title: Text(context.l10n.settings),
        ),
      ),
      PopupMenuItem(
        value: AppOverflowAction.about,
        child: ListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          leading: Icon(context.torcaIcons.info),
          title: Text(context.l10n.aboutTorca),
        ),
      ),
    ],
  );
}


