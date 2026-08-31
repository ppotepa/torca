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

class _ConversationList extends StatefulWidget {
  const _ConversationList({
    required this.conversations,
    required this.contacts,
    required this.messages,
    required this.attachments,
    required this.radio,
    required this.pinnedConversationIds,
    required this.mutedConversationIds,
    required this.draftConversationIds,
    required this.selectedConversationId,
    required this.onSelected,
    required this.onContactInfo,
    required this.onAction,
    required this.onPairContact,
  });

  final List<ConversationDto> conversations;
  final List<ContactDto> contacts;
  final List<MessageDto> messages;
  final List<AttachmentDto> attachments;
  final RadioDto radio;
  final Set<String> pinnedConversationIds;
  final Set<String> mutedConversationIds;
  final Set<String> draftConversationIds;
  final String? selectedConversationId;
  final ValueChanged<ConversationDto> onSelected;
  final ValueChanged<ContactDto> onContactInfo;
  final void Function(ConversationDto, ContactDto, ConversationAction) onAction;
  final VoidCallback onPairContact;

  @override
  State<_ConversationList> createState() => _ConversationListState();
}

class _ConversationListState extends State<_ConversationList> {
  final TextEditingController _searchController = TextEditingController();
  bool _searching = false;

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final query = _searchController.text.trim().toLowerCase();
    final filtered = query.isEmpty
        ? widget.conversations
        : widget.conversations.where(_matchesQuery).toList(growable: false);
    return Column(
      children: <Widget>[
        _searchHeader(context),
        Expanded(
          child: Column(
            children: <Widget>[
              if (query.isNotEmpty)
                Align(
                  alignment: Alignment.centerLeft,
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(16, 0, 16, 4),
                    child: Text(
                      context.strings.searchResultsCount(filtered.length),
                      style: Theme.of(context).textTheme.labelMedium,
                    ),
                  ),
                ),
              Expanded(child: _conversationResults(context, filtered, query)),
            ],
          ),
        ),
      ],
    );
  }

  Widget _searchHeader(BuildContext context) {
    if (!_searching) {
      return Padding(
        padding: const EdgeInsets.only(right: 8),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: <Widget>[
            IconButton(
              tooltip: context.strings.searchChats,
              onPressed: () => setState(() => _searching = true),
              icon: Icon(context.torcaIcons.search),
            ),
            IconButton(
              tooltip: context.strings.pairContact,
              onPressed: widget.onPairContact,
              icon: Icon(context.torcaIcons.addContact),
            ),
          ],
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 8, 8, 8),
      child: Row(
        children: <Widget>[
          Expanded(
            child: TextField(
              controller: _searchController,
              autofocus: true,
              decoration: InputDecoration(
                isDense: true,
                hintText: context.strings.searchChats,
                prefixIcon: Icon(context.torcaIcons.search),
                suffixIcon: _searchController.text.isEmpty
                    ? null
                    : IconButton(
                        tooltip: context.strings.clearSearch,
                        onPressed: () {
                          _searchController.clear();
                          setState(() {});
                        },
                        icon: Icon(context.torcaIcons.close),
                      ),
              ),
              onChanged: (_) => setState(() {}),
            ),
          ),
          IconButton(
            tooltip: context.strings.pairContact,
            onPressed: widget.onPairContact,
            icon: Icon(context.torcaIcons.addContact),
          ),
          IconButton(
            tooltip: context.strings.closeSearch,
            onPressed: () {
              _searchController.clear();
              setState(() => _searching = false);
            },
            icon: Icon(context.torcaIcons.close),
          ),
        ],
      ),
    );
  }

  Widget _conversationResults(
    BuildContext context,
    List<ConversationDto> conversations,
    String query,
  ) {
    if (conversations.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Text(
            query.isEmpty
                ? context.strings.pairContactHint
                : context.strings.noChatsMatch,
            textAlign: TextAlign.center,
          ),
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
          selected: conversation.id == widget.selectedConversationId,
          onTap: () => widget.onSelected(conversation),
          onContactInfo: contact == null
              ? null
              : () => widget.onContactInfo(contact),
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
          radio: contact == null ? null : widget.radio.forContact(contact.id),
          radioSession: widget.radio.session,
          pinned: widget.pinnedConversationIds.contains(conversation.id),
          muted: widget.mutedConversationIds.contains(conversation.id),
          draft: widget.draftConversationIds.contains(conversation.id),
        );
      },
    );
  }

  bool _matchesQuery(ConversationDto conversation) {
    final query = _searchController.text.trim().toLowerCase();
    if (query.isEmpty) return true;
    final contact = _contact(conversation.contactId);
    if (contact?.displayName.toLowerCase().contains(query) == true ||
        conversation.lastMessageBody?.toLowerCase().contains(query) == true) {
      return true;
    }
    return widget.messages.any(
          (message) =>
              message.conversationId == conversation.id &&
              message.body.toLowerCase().contains(query),
        ) ||
        widget.attachments.any(
          (attachment) =>
              widget.messages.any(
                (message) =>
                    message.id == attachment.messageId &&
                    message.conversationId == conversation.id,
              ) &&
              attachment.name.toLowerCase().contains(query),
        );
  }

  Future<void> _showActions(
    BuildContext context,
    ConversationDto conversation,
    ContactDto contact, {
    Offset? globalPosition,
  }) async {
    final blocked = contact.typedStatus == ContactStatus.blocked;
    final archived = conversation.typedStatus == ConversationStatus.archived;
    final pinned = widget.pinnedConversationIds.contains(conversation.id);
    final muted = widget.mutedConversationIds.contains(conversation.id);
    final action = globalPosition == null
        ? await ConversationActionMenu.showTouch(
            context,
            blocked: blocked,
            archived: archived,
            pinned: pinned,
            muted: muted,
            unread: conversation.unreadCount > 0,
          )
        : await ConversationActionMenu.showDesktop(
            context,
            globalPosition,
            blocked: blocked,
            archived: archived,
            pinned: pinned,
            muted: muted,
            unread: conversation.unreadCount > 0,
          );
    if (action == null || !context.mounted) return;
    if (action == ConversationAction.open) {
      widget.onSelected(conversation);
      return;
    }
    widget.onAction(conversation, contact, action);
  }

  ContactDto? _contact(String id) {
    for (final contact in widget.contacts) {
      if (contact.id == id) return contact;
    }
    return null;
  }
}

class _ContactsSection extends StatelessWidget {
  const _ContactsSection({
    required this.contacts,
    required this.pairings,
    required this.conversations,
    required this.messages,
    required this.attachments,
    required this.radio,
    required this.selectedContactId,
    required this.onOpenDetails,
    required this.onOpenConversation,
    required this.onOpenPairing,
    required this.onAction,
  });

  final List<ContactDto> contacts;
  final List<PairingDto> pairings;
  final List<ConversationDto> conversations;
  final List<MessageDto> messages;
  final List<AttachmentDto> attachments;
  final RadioDto radio;
  final String? selectedContactId;
  final ValueChanged<ContactDto> onOpenDetails;
  final ValueChanged<ContactDto> onOpenConversation;
  final Future<void> Function(PairingDto pairing) onOpenPairing;
  final Future<void> Function(ContactDto, ContactAction) onAction;

  @override
  Widget build(BuildContext context) {
    final pendingPairings = pairings
        .where(
          (pairing) =>
              pairing.typedState == PairingState.peerJoined ||
              pairing.typedState == PairingState.awaitingApproval ||
              pairing.typedState == PairingState.approved,
        )
        .toList(growable: false);
    if (contacts.isEmpty && pendingPairings.isEmpty) {
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
        final active = selected ?? contacts.firstOrNull;
        final list = ListView(
          padding: const EdgeInsets.all(16),
          children: <Widget>[
            Text(
              context.strings.contacts,
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Text(
              context.strings.contactsCount(
                contacts.length + pendingPairings.length,
              ),
            ),
            const SizedBox(height: 12),
            if (pendingPairings.isNotEmpty) ...<Widget>[
              for (final pairing in pendingPairings)
                _PendingContactCard(
                  pairing: pairing,
                  onTap: () => onOpenPairing(pairing),
                ),
              const SizedBox(height: 8),
            ],
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
                    selected: wide && contact.id == active?.id,
                    onTap: () => onOpenConversation(contact),
                    onLongPress: () => _showActions(context, contact),
                    leading: TorcaDeviceAvatar(
                      label: contact.displayName,
                      identityId: contact.remoteIdentityId,
                      fallbackIdentityId: contact.id,
                      presentation: AvatarActivityPresentation.resolve(
                        blocked: contact.typedStatus == ContactStatus.blocked,
                        online:
                            contact.typedAvailability ==
                            PeerAvailability.reachable,
                      ),
                    ),
                    title: Text(contact.displayName),
                    subtitle: Text(_contactPresence(context, contact)),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        RadioIndicator(
                          radio: radio.forContact(contact.id),
                          session: radio.session,
                          contactName: contact.displayName,
                        ),
                        ConnectionIndicator(
                          state: contact.availabilityIndicatorState,
                          blocked: contact.typedStatus == ContactStatus.blocked,
                          provider: contact.transportProvider,
                          showLabel: false,
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
        if (!wide || active == null) return list;
        return Row(
          children: <Widget>[
            SizedBox(width: 390, child: list),
            const VerticalDivider(width: 1),
            Expanded(
              child: _ContactContextPanel(
                contact: active,
                sharedAttachmentNames: _sharedAttachmentNames(active),
                onOpenConnectionDetails: () => onOpenDetails(active),
                onRename: () => onAction(active, ContactAction.rename),
                onToggleBlock: () =>
                    onAction(active, ContactAction.blockToggle),
                onRemove: () => onAction(active, ContactAction.remove),
              ),
            ),
          ],
        );
      },
    );
  }

  List<String> _sharedAttachmentNames(ContactDto contact) {
    final conversationIds = conversations
        .where((item) => item.contactId == contact.id)
        .map((item) => item.id)
        .toSet();
    final messageIds = messages
        .where((item) => conversationIds.contains(item.conversationId))
        .map((item) => item.id)
        .toSet();
    return attachments
        .where((item) => messageIds.contains(item.messageId))
        .map((item) => item.name)
        .where((item) => item.trim().isNotEmpty)
        .toList(growable: false);
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

class _PendingContactCard extends StatelessWidget {
  const _PendingContactCard({required this.pairing, required this.onTap});

  final PairingDto pairing;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final name = pairing.remoteDisplayName?.trim();
    return Card(
      child: ListTile(
        onTap: onTap,
        leading: const SizedBox(
          width: 40,
          height: 40,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
        title: Text(
          name == null || name.isEmpty ? context.strings.newContact : name,
        ),
        subtitle: Text(context.strings.finalizingContact),
        trailing: Icon(context.torcaIcons.reconnect),
      ),
    );
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
                subtitle: Text(
                  context.strings.invitationCodeLabel(pairing.code),
                ),
                trailing: Chip(
                  label: Text(
                    context.strings.pairingStateLabel(pairing.typedState),
                  ),
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
    this.sharedAttachmentNames = const <String>[],
    this.onOpenConnectionDetails,
    this.onRename,
    this.onToggleBlock,
    this.onRemove,
  });

  final ContactDto contact;
  final List<String> sharedAttachmentNames;
  final VoidCallback? onOpenConnectionDetails;
  final VoidCallback? onRename;
  final VoidCallback? onToggleBlock;
  final VoidCallback? onRemove;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.all(20),
    child: SingleChildScrollView(
      child: ContactDetailsContent(
        contact: contact,
        sharedAttachmentNames: sharedAttachmentNames,
        onOpenConnectionDetails: onOpenConnectionDetails,
        onRename: onRename,
        onToggleBlock: onToggleBlock,
        onRemove: onRemove,
        scrollable: false,
      ),
    ),
  );
}

String _contactPresence(BuildContext context, ContactDto contact) {
  if (contact.typedAvailability == PeerAvailability.reachable) {
    return context.strings.online;
  }
  if (contact.typedAvailability == PeerAvailability.unknown) {
    return context.strings.offlineShort;
  }
  final milliseconds = contact.lastSeenAtMs;
  if (milliseconds == null) return context.strings.offlineShort;
  final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
  final time =
      '${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}';
  return context.strings.lastSeenAt(time);
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
  const _ProfileSetup({
    required this.gateway,
    required this.identityId,
    this.fingerprint,
  });

  final EngineGateway gateway;
  final String? identityId;
  final String? fingerprint;

  @override
  State<_ProfileSetup> createState() => _ProfileSetupState();
}

class _ProfileSetupState extends State<_ProfileSetup> {
  final TextEditingController controller = TextEditingController();
  String? _error;
  bool _submitting = false;
  String _countryCode = 'UNKNOWN';

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final avatarSize = math
          .min(constraints.maxWidth - 48, constraints.maxHeight * 0.44)
          .clamp(160.0, 360.0);
      return Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 520),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Align(
                  child: TorcaDeviceAvatar(
                    key: const ValueKey<String>('profile-device-avatar'),
                    label: context.strings.yourIdentity,
                    identityId: widget.identityId,
                    stableDevice: true,
                    size: avatarSize,
                  ),
                ),
                const SizedBox(height: 12),
                Text(
                  context.strings.yourIdentity,
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.w800,
                    letterSpacing: 1.4,
                  ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 24),
                Text(
                  context.strings.chooseNickname,
                  style: Theme.of(context).textTheme.headlineSmall,
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 12),
                Text(
                  context.strings.nicknameIntro,
                  textAlign: TextAlign.center,
                ),
                if (widget.fingerprint != null) ...<Widget>[
                  const SizedBox(height: 16),
                  SelectableText(
                    context.strings.deviceFingerprint(widget.fingerprint!),
                    textAlign: TextAlign.center,
                  ),
                ],
                const SizedBox(height: 20),
                TextField(
                  controller: controller,
                  enabled: !_submitting,
                  decoration: InputDecoration(
                    labelText: context.strings.nickname,
                    errorText: _error,
                  ),
                  onSubmitted: _submitting ? null : (_) => _saveProfile(),
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<String>(
                  initialValue: _countryCode,
                  decoration: InputDecoration(
                    labelText: context.strings.country,
                  ),
                  items: <DropdownMenuItem<String>>[
                    DropdownMenuItem(
                      value: 'UNKNOWN',
                      child: Text('📍 ${context.strings.unknownCountry}'),
                    ),
                    DropdownMenuItem(
                      value: 'PL',
                      child: Text('🇵🇱 ${context.strings.polishCountry}'),
                    ),
                    DropdownMenuItem(
                      value: 'GB',
                      child: Text('🇬🇧 ${context.strings.englishCountry}'),
                    ),
                  ],
                  onChanged: _submitting
                      ? null
                      : (value) =>
                            setState(() => _countryCode = value ?? 'UNKNOWN'),
                ),
                const SizedBox(height: 12),
                FilledButton(
                  onPressed: _submitting ? null : _saveProfile,
                  child: Text(
                    _submitting
                        ? context.strings.saving
                        : context.strings.continueLabel,
                  ),
                ),
              ],
            ),
          ),
        ),
      );
    },
  );

  Future<void> _saveProfile() async {
    final displayName = controller.text.trim();
    if (displayName.isEmpty) {
      setState(() => _error = context.strings.nicknameRequired);
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    BridgeResultDto? result;
    Object? failure;
    try {
      final identityId = widget.identityId ?? 'local-device';
      Map<String, Object?>? avatarEnvelope;
      try {
        final avatar = await AvatarRepository.instance.envelopeForDevice(
          identityId,
        );
        avatarEnvelope =
            jsonDecode(jsonEncode(avatar.toJson())) as Map<String, Object?>;
      } on Object {
        // Avatar generation is presentation-only. A platform identifier or
        // renderer failure must never prevent saving the user's profile.
        avatarEnvelope = null;
      }
      result = await widget.gateway.execute(
        UpdateProfileCommandDto(
          displayName: displayName,
          avatarEnvelope: avatarEnvelope,
          countryCode: _countryCode,
        ),
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
                    fallback: context.strings.couldNotSaveNickname,
                  );
      });
    }
  }
}
