import 'package:flutter/material.dart';

enum ConversationAction {
  open,
  contactDetails,
  rename,
  clearHistory,
  blockToggle,
  remove,
}

abstract final class ConversationActionMenu {
  static Future<ConversationAction?> showTouch(
    BuildContext context, {
    required bool blocked,
  }) => showModalBottomSheet<ConversationAction>(
    context: context,
    builder: (context) =>
        SafeArea(child: Wrap(children: _tiles(context, blocked))),
  );

  static Future<ConversationAction?> showDesktop(
    BuildContext context,
    Offset globalPosition, {
    required bool blocked,
  }) {
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    return showMenu<ConversationAction>(
      context: context,
      position: RelativeRect.fromRect(
        Rect.fromLTWH(globalPosition.dx, globalPosition.dy, 1, 1),
        Offset.zero & overlay.size,
      ),
      items: <PopupMenuEntry<ConversationAction>>[
        _item(ConversationAction.open, Icons.forum_outlined, 'Open'),
        _item(
          ConversationAction.contactDetails,
          Icons.person_outline,
          'Contact details',
        ),
        _item(ConversationAction.rename, Icons.edit_outlined, 'Rename contact'),
        const PopupMenuDivider(),
        _item(
          ConversationAction.clearHistory,
          Icons.delete_sweep_outlined,
          'Clear conversation history',
        ),
        _item(
          ConversationAction.blockToggle,
          blocked ? Icons.check_circle_outline : Icons.block,
          blocked ? 'Unblock contact' : 'Block contact',
        ),
        const PopupMenuDivider(),
        _item(
          ConversationAction.remove,
          Icons.person_remove_outlined,
          'Remove contact',
        ),
      ],
    );
  }

  static List<Widget> _tiles(BuildContext context, bool blocked) => <Widget>[
    _tile(context, ConversationAction.open, Icons.forum_outlined, 'Open'),
    _tile(
      context,
      ConversationAction.contactDetails,
      Icons.person_outline,
      'Contact details',
    ),
    _tile(
      context,
      ConversationAction.rename,
      Icons.edit_outlined,
      'Rename contact',
    ),
    const Divider(height: 1),
    _tile(
      context,
      ConversationAction.clearHistory,
      Icons.delete_sweep_outlined,
      'Clear conversation history',
    ),
    _tile(
      context,
      ConversationAction.blockToggle,
      blocked ? Icons.check_circle_outline : Icons.block,
      blocked ? 'Unblock contact' : 'Block contact',
    ),
    const Divider(height: 1),
    _tile(
      context,
      ConversationAction.remove,
      Icons.person_remove_outlined,
      'Remove contact',
    ),
  ];

  static PopupMenuItem<ConversationAction> _item(
    ConversationAction action,
    IconData icon,
    String label,
  ) => PopupMenuItem<ConversationAction>(
    value: action,
    child: ListTile(
      dense: true,
      contentPadding: EdgeInsets.zero,
      leading: Icon(icon),
      title: Text(label),
    ),
  );

  static Widget _tile(
    BuildContext context,
    ConversationAction action,
    IconData icon,
    String label,
  ) => ListTile(
    leading: Icon(icon),
    title: Text(label),
    onTap: () => Navigator.of(context).pop(action),
  );
}
