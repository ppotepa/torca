import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';

enum MessageAction { reply, copy, details }

abstract final class MessageActionMenu {
  static Future<MessageAction?> showTouch(BuildContext context) =>
      showModalBottomSheet<MessageAction>(
        context: context,
        builder: (context) => SafeArea(
          child: Wrap(
            children: <Widget>[
              _tile(
                context,
                MessageAction.reply,
                context.torcaIcons.reply,
                context.strings.reply,
              ),
              _tile(
                context,
                MessageAction.copy,
                context.torcaIcons.copy,
                context.strings.copy,
              ),
              _tile(
                context,
                MessageAction.details,
                context.torcaIcons.info,
                context.strings.messageDetails,
              ),
            ],
          ),
        ),
      );

  static Future<MessageAction?> showDesktop(
    BuildContext context,
    Offset globalPosition,
  ) {
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    return showMenu<MessageAction>(
      context: context,
      position: RelativeRect.fromRect(
        Rect.fromLTWH(globalPosition.dx, globalPosition.dy, 1, 1),
        Offset.zero & overlay.size,
      ),
      items: <PopupMenuEntry<MessageAction>>[
        PopupMenuItem(
          value: MessageAction.reply,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.reply),
            title: Text(context.strings.reply),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.copy,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.copy),
            title: Text(context.strings.copy),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.details,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.info),
            title: Text(context.strings.messageDetails),
          ),
        ),
      ],
    );
  }

  static Widget _tile(
    BuildContext context,
    MessageAction action,
    IconData icon,
    String label,
  ) => ListTile(
    leading: Icon(icon),
    title: Text(label),
    onTap: () => Navigator.of(context).pop(action),
  );
}
