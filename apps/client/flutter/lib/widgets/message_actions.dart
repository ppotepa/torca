import 'dart:async';

import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

enum MessageAction {
  reply,
  react,
  copy,
  forward,
  bookmark,
  edit,
  cancel,
  delete,
  details,
}

abstract final class MessageActionMenu {
  static const List<String> quickReactions = <String>[
    '\u{1F44D}',
    '\u{2764}\u{FE0F}',
    '\u{1F602}',
    '\u{1F62E}',
    '\u{1F622}',
    '\u{1F64F}',
  ];

  static Future<MessageAction?> showTouch(
    BuildContext context, {
    bool canCancel = false,
    bool canEdit = false,
    bool canDelete = false,
    bool bookmarked = false,
    FutureOr<void> Function(String)? onQuickReaction,
  }) => showModalBottomSheet<MessageAction>(
    context: context,
    builder: (context) => SafeArea(
      child: Wrap(
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
            child: Row(
              children: quickReactions
                  .map(
                    (emoji) => Expanded(
                      child: Semantics(
                        button: true,
                        label: '${context.l10n.reactToMessage} $emoji',
                        child: InkWell(
                          borderRadius: BorderRadius.circular(12),
                          onTap: () async {
                            await onQuickReaction?.call(emoji);
                            if (context.mounted) {
                              Navigator.of(context).pop(MessageAction.react);
                            }
                          },
                          child: Padding(
                            padding: const EdgeInsets.all(10),
                            child: Text(
                              emoji,
                              textAlign: TextAlign.center,
                              style: const TextStyle(fontSize: 22),
                            ),
                          ),
                        ),
                      ),
                    ),
                  )
                  .toList(growable: false),
            ),
          ),
          _tile(
            context,
            MessageAction.reply,
            context.torcaIcons.reply,
            context.l10n.reply,
          ),
          _tile(
            context,
            MessageAction.react,
            context.torcaIcons.emoji,
            context.l10n.reactToMessage,
          ),
          _tile(
            context,
            MessageAction.copy,
            context.torcaIcons.copy,
            context.l10n.copy,
          ),
          _tile(
            context,
            MessageAction.forward,
            context.torcaIcons.forward,
            context.l10n.forwardMessage,
          ),
          _tile(
            context,
            MessageAction.bookmark,
            context.torcaIcons.bookmark,
            bookmarked
                ? context.l10n.removeBookmark
                : context.l10n.bookmarkMessage,
          ),
          if (canCancel)
            _tile(
              context,
              MessageAction.cancel,
              context.torcaIcons.close,
              context.l10n.cancelMessage,
            ),
          if (canEdit)
            _tile(
              context,
              MessageAction.edit,
              context.torcaIcons.edit,
              context.l10n.editMessage,
            ),
          if (canDelete)
            _tile(
              context,
              MessageAction.delete,
              context.torcaIcons.remove,
              context.l10n.deleteMessage,
            ),
          _tile(
            context,
            MessageAction.details,
            context.torcaIcons.info,
            context.l10n.messageDetails,
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
    bool canDelete = false,
    bool bookmarked = false,
    FutureOr<void> Function(String)? onQuickReaction,
  }) {
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    return showMenu<MessageAction>(
      context: context,
      position: RelativeRect.fromRect(
        Rect.fromLTWH(globalPosition.dx, globalPosition.dy, 1, 1),
        Offset.zero & overlay.size,
      ),
      items: <PopupMenuEntry<MessageAction>>[
        PopupMenuItem<MessageAction>(
          value: MessageAction.react,
          padding: const EdgeInsets.fromLTRB(8, 4, 8, 4),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              for (final emoji in quickReactions)
                IconButton(
                  tooltip: '${context.l10n.reactToMessage} $emoji',
                  visualDensity: VisualDensity.compact,
                  constraints: const BoxConstraints(
                    minWidth: 34,
                    minHeight: 34,
                  ),
                  padding: EdgeInsets.zero,
                  onPressed: onQuickReaction == null
                      ? null
                      : () async {
                          await onQuickReaction(emoji);
                          Navigator.of(context).pop();
                        },
                  icon: Text(emoji, style: const TextStyle(fontSize: 20)),
                ),
            ],
          ),
        ),
        PopupMenuItem(
          value: MessageAction.reply,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.reply),
            title: Text(context.l10n.reply),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.react,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.emoji),
            title: Text(context.l10n.reactToMessage),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.forward,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.forward),
            title: Text(context.l10n.forwardMessage),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.copy,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.copy),
            title: Text(context.l10n.copy),
          ),
        ),
        PopupMenuItem(
          value: MessageAction.bookmark,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.bookmark),
            title: Text(
              bookmarked
                  ? context.l10n.removeBookmark
                  : context.l10n.bookmarkMessage,
            ),
          ),
        ),
        if (canCancel)
          PopupMenuItem(
            value: MessageAction.cancel,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(context.torcaIcons.close),
              title: Text(context.l10n.cancelMessage),
            ),
          ),
        if (canEdit)
          PopupMenuItem(
            value: MessageAction.edit,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(context.torcaIcons.edit),
              title: Text(context.l10n.editMessage),
            ),
          ),
        if (canDelete)
          PopupMenuItem(
            value: MessageAction.delete,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(context.torcaIcons.remove),
              title: Text(context.l10n.deleteMessage),
            ),
          ),
        PopupMenuItem(
          value: MessageAction.details,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(context.torcaIcons.info),
            title: Text(context.l10n.messageDetails),
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
