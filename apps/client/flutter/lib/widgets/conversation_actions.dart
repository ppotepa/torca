import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';

enum ConversationAction {
  open,
  markRead,
  contactDetails,
  rename,
  clearHistory,
  archive,
  restore,
  pinToggle,
  muteToggle,
  blockToggle,
  remove,
}

/// Actions available directly from a contact.  Contacts without a previously
/// opened conversation deliberately do not expose history-specific actions.
enum ContactAction {
  open,
  contactDetails,
  connectionDetails,
  rename,
  blockToggle,
  remove,
}

abstract final class ContactActionMenu {
  static Future<ContactAction?> showTouch(
    BuildContext context, {
    required bool blocked,
  }) => showModalBottomSheet<ContactAction>(
    context: context,
    useSafeArea: true,
    isScrollControlled: false,
    enableDrag: false,
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
            ContactAction.connectionDetails,
            context.torcaIcons.diagnostics,
            context.strings.connectionDetails,
          ),
          _tile(
            context,
            ContactAction.rename,
            context.torcaIcons.edit,
            context.strings.renameContact,
          ),
          const Divider(height: 1),
          _tile(
            context,
            ContactAction.blockToggle,
            blocked ? context.torcaIcons.success : context.torcaIcons.block,
            blocked
                ? context.strings.unblockContact
                : context.strings.blockContact,
          ),
          const Divider(height: 1),
          _tile(
            context,
            ContactAction.remove,
            context.torcaIcons.remove,
            context.strings.removeContact,
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
        _item(
          ContactAction.connectionDetails,
          context.torcaIcons.diagnostics,
          context.strings.connectionDetails,
        ),
        _item(
          ContactAction.rename,
          context.torcaIcons.edit,
          context.strings.renameContact,
        ),
        const PopupMenuDivider(),
        _item(
          ContactAction.blockToggle,
          blocked ? context.torcaIcons.success : context.torcaIcons.block,
          blocked
              ? context.strings.unblockContact
              : context.strings.blockContact,
        ),
        const PopupMenuDivider(),
        _item(
          ContactAction.remove,
          context.torcaIcons.remove,
          context.strings.removeContact,
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
    required bool archived,
    required bool pinned,
    required bool muted,
    required bool unread,
  }) => showModalBottomSheet<ConversationAction>(
    context: context,
    useSafeArea: true,
    isScrollControlled: false,
    enableDrag: false,
    builder: (context) => SafeArea(
      child: Wrap(
        children: _tiles(context, blocked, archived, pinned, muted, unread),
      ),
    ),
  );

  static Future<ConversationAction?> showDesktop(
    BuildContext context,
    Offset globalPosition, {
    required bool blocked,
    required bool archived,
    required bool pinned,
    required bool muted,
    required bool unread,
  }) {
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    return showMenu<ConversationAction>(
      context: context,
      position: RelativeRect.fromRect(
        Rect.fromLTWH(globalPosition.dx, globalPosition.dy, 1, 1),
        Offset.zero & overlay.size,
      ),
      items: <PopupMenuEntry<ConversationAction>>[
        _item(
          ConversationAction.open,
          context.torcaIcons.chats,
          context.strings.open,
        ),
        if (unread)
          _item(
            ConversationAction.markRead,
            context.torcaIcons.read,
            context.strings.markConversationRead,
          ),
        _item(
          ConversationAction.contactDetails,
          context.torcaIcons.contacts,
          context.strings.contactInformation,
        ),
        _item(
          ConversationAction.rename,
          context.torcaIcons.edit,
          context.strings.renameContact,
        ),
        const PopupMenuDivider(),
        _item(
          archived ? ConversationAction.restore : ConversationAction.archive,
          context.torcaIcons.archive,
          archived
              ? context.strings.restoreConversation
              : context.strings.archiveConversation,
        ),
        _item(
          ConversationAction.pinToggle,
          context.torcaIcons.pin,
          pinned
              ? context.strings.unpinConversation
              : context.strings.pinConversation,
        ),
        _item(
          ConversationAction.muteToggle,
          context.torcaIcons.notifications,
          muted
              ? context.strings.unmuteConversation
              : context.strings.muteConversation,
        ),
        if (!archived)
          _item(
            ConversationAction.clearHistory,
            context.torcaIcons.remove,
            context.strings.clearConversationHistory,
          ),
        _item(
          ConversationAction.blockToggle,
          blocked ? context.torcaIcons.success : context.torcaIcons.block,
          blocked
              ? context.strings.unblockContact
              : context.strings.blockContact,
        ),
        const PopupMenuDivider(),
        _item(
          ConversationAction.remove,
          context.torcaIcons.remove,
          context.strings.removeContact,
        ),
      ],
    );
  }

  static List<Widget> _tiles(
    BuildContext context,
    bool blocked,
    bool archived,
    bool pinned,
    bool muted,
    bool unread,
  ) => <Widget>[
    _tile(
      context,
      ConversationAction.open,
      context.torcaIcons.chats,
      context.strings.open,
    ),
    if (unread)
      _tile(
        context,
        ConversationAction.markRead,
        context.torcaIcons.read,
        context.strings.markConversationRead,
      ),
    _tile(
      context,
      ConversationAction.pinToggle,
      context.torcaIcons.pin,
      pinned
          ? context.strings.unpinConversation
          : context.strings.pinConversation,
    ),
    _tile(
      context,
      ConversationAction.muteToggle,
      context.torcaIcons.notifications,
      muted
          ? context.strings.unmuteConversation
          : context.strings.muteConversation,
    ),
    _tile(
      context,
      ConversationAction.contactDetails,
      context.torcaIcons.contacts,
      context.strings.contactInformation,
    ),
    _tile(
      context,
      ConversationAction.rename,
      context.torcaIcons.edit,
      context.strings.renameContact,
    ),
    const Divider(height: 1),
    _tile(
      context,
      archived ? ConversationAction.restore : ConversationAction.archive,
      context.torcaIcons.archive,
      archived
          ? context.strings.restoreConversation
          : context.strings.archiveConversation,
    ),
    if (!archived)
      _tile(
        context,
        ConversationAction.clearHistory,
        context.torcaIcons.remove,
        context.strings.clearConversationHistory,
      ),
    _tile(
      context,
      ConversationAction.blockToggle,
      blocked ? context.torcaIcons.success : context.torcaIcons.block,
      blocked ? context.strings.unblockContact : context.strings.blockContact,
    ),
    const Divider(height: 1),
    _tile(
      context,
      ConversationAction.remove,
      context.torcaIcons.remove,
      context.strings.removeContact,
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
