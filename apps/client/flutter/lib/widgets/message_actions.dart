import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';

enum MessageAction { reply, react, copy, forward, edit, cancel, details }

abstract final class MessageActionMenu {
  static Future<MessageAction?> showTouch(
    BuildContext context, {
    bool canCancel = false,
    bool canEdit = false,
  }) => showModalBottomSheet<MessageAction>(
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
            MessageAction.react,
            context.torcaIcons.success,
            context.strings.reactToMessage,
          ),
          _tile(
            context,
            MessageAction.copy,
            context.torcaIcons.copy,
            context.strings.copy,
          ),
          _tile(
            context,
            MessageAction.forward,
            context.torcaIcons.forward,
            context.strings.forwardMessage,
          ),
          if (canCancel)
            _tile(
              context,
              MessageAction.cancel,
              context.torcaIcons.close,
              context.strings.cancelMessage,
            ),
          if (canEdit)
            _tile(
              context,
              MessageAction.edit,
              context.torcaIcons.edit,
              context.strings.editMessage,
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
    Offset globalPosition, {
    bool canCancel = false,
    bool canEdit = false,
  }) {
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
          value: MessageAction.react,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.success),
            title: Text(context.strings.reactToMessage),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.forward,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.forward),
            title: Text(context.strings.forwardMessage),
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
        if (canCancel)
          PopupMenuItem(
            value: MessageAction.cancel,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(context.torcaIcons.close),
              title: Text(context.strings.cancelMessage),
            ),
          ),
        if (canEdit)
          PopupMenuItem(
            value: MessageAction.edit,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(context.torcaIcons.edit),
              title: Text(context.strings.editMessage),
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
