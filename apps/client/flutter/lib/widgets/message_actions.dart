import 'package:flutter/material.dart';

enum MessageAction { reply, copy, details }

abstract final class MessageActionMenu {
  static Future<MessageAction?> showTouch(BuildContext context) =>
      showModalBottomSheet<MessageAction>(
        context: context,
        builder: (context) => SafeArea(
          child: Wrap(
            children: <Widget>[
              _tile(context, MessageAction.reply, Icons.reply, 'Reply'),
              _tile(context, MessageAction.copy, Icons.copy_outlined, 'Copy'),
              _tile(
                context,
                MessageAction.details,
                Icons.info_outline,
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
      items: const <PopupMenuEntry<MessageAction>>[
        PopupMenuItem(
          value: MessageAction.reply,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.reply),
            title: Text('Reply'),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.copy,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.copy_outlined),
            title: Text('Copy'),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.details,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.info_outline),
            title: Text('Message details'),
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
