part of 'home_screen.dart';

class _NavigationIcon extends StatelessWidget {
  const _NavigationIcon({required this.icon, required this.count});

  final IconData icon;
  final int count;

  @override
  Widget build(BuildContext context) => Stack(
    clipBehavior: Clip.none,
    children: <Widget>[
      Icon(icon),
      if (count > 0)
        Positioned(
          right: -12,
          top: -9,
          child: TorcaBadge(label: Text('${count > 99 ? 99 : count}')),
        ),
    ],
  );
}

class _PaneDivider extends StatelessWidget {
  const _PaneDivider({required this.onDrag});

  final ValueChanged<double> onDrag;

  @override
  Widget build(BuildContext context) => MouseRegion(
    cursor: SystemMouseCursors.resizeColumn,
    child: GestureDetector(
      behavior: HitTestBehavior.opaque,
      onHorizontalDragUpdate: (details) => onDrag(details.delta.dx),
      child: SizedBox(
        width: 7,
        child: Center(
          child: Container(width: 1, color: Theme.of(context).dividerColor),
        ),
      ),
    ),
  );
}

class _ConversationList extends StatelessWidget {
  const _ConversationList({
    required this.conversations,
    required this.contacts,
    required this.selectedConversationId,
    required this.onSelected,
    required this.onContactInfo,
    required this.onAction,
  });

  final List<ConversationDto> conversations;
  final List<ContactDto> contacts;
  final String? selectedConversationId;
  final ValueChanged<ConversationDto> onSelected;
  final ValueChanged<ContactDto> onContactInfo;
  final void Function(ConversationDto, ContactDto, ConversationAction) onAction;

  @override
  Widget build(BuildContext context) {
    if (conversations.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Text(context.strings.pairContactHint),
        ),
      );
    }
    return ListView.builder(
      itemCount: conversations.length,
      itemBuilder: (context, index) {
        final conversation = conversations[index];
        final contact = _contact(conversation.contactId);
        return ConversationSummaryTile(
          conversation: conversation,
          contact: contact,
          selected: conversation.id == selectedConversationId,
          onTap: () => onSelected(conversation),
          onContactInfo: contact == null ? null : () => onContactInfo(contact),
          onLongPress: contact == null
              ? null
              : () => _showActions(context, conversation, contact),
          onSecondaryTapDown: contact == null
              ? null
              : (details) => _showActions(
                  context,
                  conversation,
                  contact,
                  globalPosition: details.globalPosition,
                ),
        );
      },
    );
  }

  Future<void> _showActions(
    BuildContext context,
    ConversationDto conversation,
    ContactDto contact, {
    Offset? globalPosition,
  }) async {
    final blocked = contact.typedStatus == ContactStatus.blocked;
    final action = globalPosition == null
        ? await ConversationActionMenu.showTouch(context, blocked: blocked)
        : await ConversationActionMenu.showDesktop(
            context,
            globalPosition,
            blocked: blocked,
          );
    if (action == null || !context.mounted) return;
    if (action == ConversationAction.open) {
      onSelected(conversation);
      return;
    }
    onAction(conversation, contact, action);
  }

  ContactDto? _contact(String id) {
    for (final contact in contacts) {
      if (contact.id == id) return contact;
    }
    return null;
  }
}

class _ContactsSection extends StatelessWidget {
  const _ContactsSection({
    required this.contacts,
    required this.selectedContactId,
    required this.onOpenDetails,
    required this.onOpenConversation,
    required this.onAction,
  });

  final List<ContactDto> contacts;
  final String? selectedContactId;
  final ValueChanged<ContactDto> onOpenDetails;
  final ValueChanged<ContactDto> onOpenConversation;
  final Future<void> Function(ContactDto, ContactAction) onAction;

  @override
  Widget build(BuildContext context) {
    if (contacts.isEmpty) {
      return _SectionEmptyState(
        icon: context.torcaIcons.contacts,
        title: context.strings.noContactsYet,
        message: context.strings.createInvitationForContact,
      );
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        final wide = constraints.maxWidth >= _wideLayoutBreakpoint;
        ContactDto? selected;
        for (final contact in contacts) {
          if (contact.id == selectedContactId) {
            selected = contact;
            break;
          }
        }
        final active = selected ?? contacts.first;
        final list = ListView(
          padding: const EdgeInsets.all(16),
          children: <Widget>[
            Text(
              context.strings.contacts,
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Text(context.strings.contactsCount(contacts.length)),
            const SizedBox(height: 12),
            for (final contact in contacts)
              GestureDetector(
                behavior: HitTestBehavior.translucent,
                onSecondaryTapDown: (details) => _showActions(
                  context,
                  contact,
                  globalPosition: details.globalPosition,
                ),
                child: Card(
                  clipBehavior: Clip.antiAlias,
                  child: ListTile(
                    selected: wide && contact.id == active.id,
                    onTap: () => onOpenConversation(contact),
                    onLongPress: () => _showActions(context, contact),
                    leading: TorcaAvatar(label: contact.displayName),
                    title: Text(contact.displayName),
                    subtitle: Text(_contactPresence(contact)),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        ConnectionIndicator(
                          state: contact.connectionState,
                          blocked: contact.typedStatus == ContactStatus.blocked,
                          showLabel: false,
                        ),
                        IconButton(
                          tooltip: context.strings.openChat,
                          onPressed: () => onOpenConversation(contact),
                          icon: Icon(context.torcaIcons.chats),
                        ),
                        IconButton(
                          tooltip: context.strings.contactInformation,
                          onPressed: () => onOpenDetails(contact),
                          icon: Icon(context.torcaIcons.contactInfo),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
          ],
        );
        if (!wide) return list;
        return Row(
          children: <Widget>[
            SizedBox(width: 390, child: list),
            const VerticalDivider(width: 1),
            Expanded(
              child: _ContactContextPanel(
                contact: active,
                onOpenConversation: () => onOpenConversation(active),
              ),
            ),
          ],
        );
      },
    );
  }

  Future<void> _showActions(
    BuildContext context,
    ContactDto contact, {
    Offset? globalPosition,
  }) async {
    final action = globalPosition == null
        ? await ContactActionMenu.showTouch(
            context,
            blocked: contact.typedStatus == ContactStatus.blocked,
          )
        : await ContactActionMenu.showDesktop(
            context,
            globalPosition,
            blocked: contact.typedStatus == ContactStatus.blocked,
          );
    if (action != null && context.mounted) await onAction(contact, action);
  }
}

class _InvitationsSection extends StatelessWidget {
  const _InvitationsSection({
    required this.pairings,
    required this.onOpen,
    required this.onOpenInvitation,
  });

  final List<PairingDto> pairings;
  final VoidCallback onOpen;
  final Future<void> Function(PairingDto pairing) onOpenInvitation;

  @override
  Widget build(BuildContext context) {
    // Historical sessions remain in the runtime audit projection, but should
    // not occupy the actionable Invitations list after acceptance/expiry.
    final activePairings = pairings
        .where(
          (pairing) => switch (pairing.typedState) {
            PairingState.completed ||
            PairingState.rejected ||
            PairingState.cancelled ||
            PairingState.expired => false,
            _ => true,
          },
        )
        .toList(growable: false);
    return ListView(
    padding: const EdgeInsets.all(24),
    children: <Widget>[
      Text(
        context.strings.invitations,
        style: Theme.of(context).textTheme.headlineSmall,
      ),
      const SizedBox(height: 8),
      Text(context.strings.createManageInvitations),
      const SizedBox(height: 20),
      FilledButton.icon(
        onPressed: onOpen,
        icon: Icon(context.torcaIcons.invitations),
        label: Text(context.strings.generateInvitation),
      ),
      const SizedBox(height: 24),
      if (activePairings.isEmpty)
        _SectionEmptyState(
          icon: context.torcaIcons.invitations,
          title: context.strings.noInvitations,
          message: context.strings.activeInvitationsDescription,
        )
      else ...<Widget>[
        Text(
          context.strings.recentInvitations,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 8),
        for (final pairing in activePairings.reversed)
          Card(
            child: ListTile(
              leading: Icon(
                pairing.typedRole == PairingRole.creator
                    ? context.torcaIcons.invitations
                    : context.torcaIcons.link,
              ),
              title: Text(
                pairing.typedRole == PairingRole.creator
                    ? context.strings.createdInvitation
                    : context.strings.joinedInvitation,
              ),
              subtitle: Text(context.strings.invitationCode(pairing.code)),
              trailing: Chip(
                label: Text(context.strings.pairingStateLabel(pairing.typedState)),
              ),
              onTap: () => onOpenInvitation(pairing),
            ),
          ),
      ],
    ],
  );
  }
}

class _ContactContextPanel extends StatelessWidget {
  const _ContactContextPanel({
    required this.contact,
    required this.onOpenConversation,
  });

  final ContactDto contact;
  final VoidCallback onOpenConversation;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.all(20),
    child: SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          TorcaAvatar(label: contact.displayName, size: 56),
          const SizedBox(height: 14),
          Text(
            contact.displayName,
            style: Theme.of(context).textTheme.titleLarge,
          ),
          const SizedBox(height: 4),
          Text(_contactPresence(contact)),
          const SizedBox(height: 20),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              FilledButton.icon(
                onPressed: contact.typedStatus == ContactStatus.blocked
                    ? null
                    : onOpenConversation,
                icon: Icon(context.torcaIcons.chats),
                label: Text(context.strings.openChat),
              ),
            ],
          ),
          const SizedBox(height: 20),
          Text(
            context.strings.connection,
            style: const TextStyle(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 6),
          const SizedBox(height: 8),
          ConnectionIndicator(
            state: contact.connectionState,
            blocked: contact.typedStatus == ContactStatus.blocked,
          ),
          const SizedBox(height: 16),
          _ContextValue(
            label: context.strings.quality,
            value: contact.peerHealth.quality,
          ),
          _ContextValue(
            label: 'Round trip',
            value: contact.peerHealth.rttMs == null
                ? context.strings.notMeasured
                : '${contact.peerHealth.rttMs} ms',
          ),
          _ContextValue(
            label: context.strings.presence,
            value: contact.presenceState,
          ),
          _ContextValue(
            label: context.strings.lastSeen,
            value: contact.lastSeenAtMs == null
                ? context.strings.never
                : _formatLastSeenDetails(contact.lastSeenAtMs!),
          ),
        ],
      ),
    ),
  );
}

class _ContextValue extends StatelessWidget {
  const _ContextValue({required this.label, required this.value});
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 10),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        SizedBox(
          width: 92,
          child: Text(label, style: Theme.of(context).textTheme.labelMedium),
        ),
        Expanded(child: Text(value)),
      ],
    ),
  );
}

String _contactPresence(ContactDto contact) {
  if (contact.presenceState == 'online') return 'Online';
  final milliseconds = contact.lastSeenAtMs;
  if (milliseconds == null) return 'Offline';
  final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
  return 'Last seen ${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}';
}

String _formatLastSeenDetails(int milliseconds) {
  final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
  final hour = date.hour.toString().padLeft(2, '0');
  final minute = date.minute.toString().padLeft(2, '0');
  return '${date.year.toString().padLeft(4, '0')}-'
      '${date.month.toString().padLeft(2, '0')}-'
      '${date.day.toString().padLeft(2, '0')} $hour:$minute';
}

class _SectionEmptyState extends StatelessWidget {
  const _SectionEmptyState({
    required this.icon,
    required this.title,
    required this.message,
  });

  final IconData icon;
  final String title;
  final String message;

  @override
  Widget build(BuildContext context) => Center(
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 340),
      child: Padding(
        padding: const EdgeInsets.all(28),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(icon, size: 52, color: Theme.of(context).colorScheme.primary),
            const SizedBox(height: 16),
            Text(title, style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 8),
            Text(message, textAlign: TextAlign.center),
          ],
        ),
      ),
    ),
  );
}

class _ConversationPlaceholder extends StatelessWidget {
  const _ConversationPlaceholder();

  @override
  Widget build(BuildContext context) => Center(
    child: Column(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Icon(context.torcaIcons.chats, size: 48),
        const SizedBox(height: 12),
        Text(context.strings.selectConversation),
      ],
    ),
  );
}

class _ProfileSetup extends StatefulWidget {
  const _ProfileSetup({required this.gateway, this.fingerprint});

  final EngineGateway gateway;
  final String? fingerprint;

  @override
  State<_ProfileSetup> createState() => _ProfileSetupState();
}

class _ProfileSetupState extends State<_ProfileSetup> {
  final TextEditingController controller = TextEditingController();
  String? _error;
  bool _submitting = false;

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Center(
    child: SingleChildScrollView(
      padding: const EdgeInsets.all(24),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 420),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              'Choose your nickname',
              style: Theme.of(context).textTheme.headlineSmall,
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            const Text(
              'The secure Tor network is ready. This name will be shown to contacts.',
              textAlign: TextAlign.center,
            ),
            if (widget.fingerprint != null) ...<Widget>[
              const SizedBox(height: 16),
              SelectableText(
                'Device fingerprint\n${widget.fingerprint}',
                textAlign: TextAlign.center,
              ),
            ],
            const SizedBox(height: 20),
            TextField(
              controller: controller,
              enabled: !_submitting,
              decoration: InputDecoration(
                labelText: 'Nickname',
                errorText: _error,
              ),
              onSubmitted: _submitting ? null : (_) => _saveProfile(),
            ),
            const SizedBox(height: 12),
            FilledButton(
              onPressed: _submitting ? null : _saveProfile,
              child: Text(_submitting ? 'Saving...' : 'Continue'),
            ),
          ],
        ),
      ),
    ),
  );

  Future<void> _saveProfile() async {
    final displayName = controller.text.trim();
    if (displayName.isEmpty) {
      setState(() => _error = 'Nickname is required');
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    BridgeResultDto? result;
    Object? failure;
    try {
      result = await widget.gateway.execute(
        UpdateProfileCommandDto(displayName: displayName),
      );
    } on Object catch (error) {
      failure = error;
    } finally {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = failure == null && result != null && result.ok
            ? null
            : failure?.toString() ??
                  BridgeErrorPresenter.localized(
                    context,
                    result ?? const BridgeResultDto(ok: false, kind: 'error'),
                    fallback: 'Could not save nickname',
                  );
      });
    }
  }
}
