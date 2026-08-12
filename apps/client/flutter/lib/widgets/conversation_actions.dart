import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';

enum ConversationAction {
  open,
  contactDetails,
  rename,
  clearHistory,
  blockToggle,
  remove,
}

/// Actions available directly from a contact.  Contacts without a previously
/// opened conversation deliberately do not expose history-specific actions.
enum ContactAction { open, contactDetails, rename, blockToggle, remove }

abstract final class ContactActionMenu {
  static Future<ContactAction?> showTouch(
    BuildContext context, {
    required bool blocked,
  }) => showModalBottomSheet<ContactAction>(
    context: context,
    builder: (context) => SafeArea(
      child: Wrap(
        children: <Widget>[
          _tile(
            context,
            ContactAction.open,
            context.torcaIcons.chats,
            context.strings.openChat,
          ),
          _tile(
            context,
            ContactAction.contactDetails,
            context.torcaIcons.contactInfo,
            context.strings.contactInformation,
          ),
          _tile(
            context,
            ContactAction.rename,
            context.torcaIcons.edit,
            'Rename contact',
          ),
          const Divider(height: 1),
          _tile(
            context,
            ContactAction.blockToggle,
            blocked ? context.torcaIcons.success : context.torcaIcons.block,
            blocked ? 'Unblock contact' : 'Block contact',
          ),
          const Divider(height: 1),
          _tile(
            context,
            ContactAction.remove,
            context.torcaIcons.remove,
            'Remove contact',
          ),
        ],
      ),
    ),
  );

  static Future<ContactAction?> showDesktop(
    BuildContext context,
    Offset globalPosition, {
    required bool blocked,
  }) {
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    return showMenu<ContactAction>(
      context: context,
      position: RelativeRect.fromRect(
        Rect.fromLTWH(globalPosition.dx, globalPosition.dy, 1, 1),
        Offset.zero & overlay.size,
      ),
      items: <PopupMenuEntry<ContactAction>>[
        _item(
          ContactAction.open,
          context.torcaIcons.chats,
          context.strings.openChat,
        ),
        _item(
          ContactAction.contactDetails,
          context.torcaIcons.contactInfo,
          context.strings.contactInformation,
        ),
        _item(ContactAction.rename, context.torcaIcons.edit, 'Rename contact'),
        const PopupMenuDivider(),
        _item(
          ContactAction.blockToggle,
          blocked ? context.torcaIcons.success : context.torcaIcons.block,
          blocked ? 'Unblock contact' : 'Block contact',
        ),
        const PopupMenuDivider(),
        _item(
          ContactAction.remove,
          context.torcaIcons.remove,
          'Remove contact',
        ),
      ],
    );
  }

  static PopupMenuItem<ContactAction> _item(
    ContactAction action,
    IconData icon,
    String label,
  ) => PopupMenuItem<ContactAction>(
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
    ContactAction action,
    IconData icon,
    String label,
  ) => ListTile(
    leading: Icon(icon),
    title: Text(label),
    onTap: () => Navigator.of(context).pop(action),
  );
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
        _item(ConversationAction.open, context.torcaIcons.chats, 'Open'),
        _item(
          ConversationAction.contactDetails,
          context.torcaIcons.contacts,
          'Contact details',
        ),
        _item(
          ConversationAction.rename,
          context.torcaIcons.edit,
          'Rename contact',
        ),
        const PopupMenuDivider(),
        _item(
          ConversationAction.clearHistory,
          context.torcaIcons.remove,
          'Clear conversation history',
        ),
        _item(
          ConversationAction.blockToggle,
          blocked ? context.torcaIcons.success : context.torcaIcons.block,
          blocked ? 'Unblock contact' : 'Block contact',
        ),
        const PopupMenuDivider(),
        _item(
          ConversationAction.remove,
          context.torcaIcons.remove,
          'Remove contact',
        ),
      ],
    );
  }

  static List<Widget> _tiles(BuildContext context, bool blocked) => <Widget>[
    _tile(context, ConversationAction.open, context.torcaIcons.chats, 'Open'),
    _tile(
      context,
      ConversationAction.contactDetails,
      context.torcaIcons.contacts,
      'Contact details',
    ),
    _tile(
      context,
      ConversationAction.rename,
      context.torcaIcons.edit,
      'Rename contact',
    ),
    const Divider(height: 1),
    _tile(
      context,
      ConversationAction.clearHistory,
      context.torcaIcons.remove,
      'Clear conversation history',
    ),
    _tile(
      context,
      ConversationAction.blockToggle,
      blocked ? context.torcaIcons.success : context.torcaIcons.block,
      blocked ? 'Unblock contact' : 'Block contact',
    ),
    const Divider(height: 1),
    _tile(
      context,
      ConversationAction.remove,
      context.torcaIcons.remove,
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
