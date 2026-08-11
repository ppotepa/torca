import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

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
                'Reply',
              ),
              _tile(
                context,
                MessageAction.copy,
                context.torcaIcons.copy,
                'Copy',
              ),
              _tile(
                context,
                MessageAction.details,
                context.torcaIcons.info,
                'Message details',
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
            title: const Text('Reply'),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.copy,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.copy),
            title: const Text('Copy'),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.details,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.info),
            title: const Text('Message details'),
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
