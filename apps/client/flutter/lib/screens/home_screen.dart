import 'dart:math';

import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../settings/local_preferences.dart';
import '../widgets/app_overflow_menu.dart';
import '../widgets/connection_indicator.dart';
import '../widgets/conversation_actions.dart';
import '../widgets/tor_status_indicator.dart';
import 'contact_details_screen.dart';
import 'conversation_screen.dart';
import 'diagnostics_screen.dart';
import 'pairing_screen.dart';
import 'settings_screen.dart';

const double _wideLayoutBreakpoint = 900;
const double _conversationRailWidth = 360;

class HomeScreen extends StatefulWidget {
  const HomeScreen({required this.gateway, required this.preferences, super.key});
  final EngineGateway gateway;
  final LocalPreferences preferences;
  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  String? _selectedConversationId;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
        valueListenable: widget.gateway.snapshots,
        builder: (context, snapshot, _) => Scaffold(
          appBar: AppBar(
            title: Text(snapshot.identity?.displayName ?? 'Torca'),
            actions: <Widget>[
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 10),
                child: TorStatusIndicator(state: snapshot.torState),
              ),
              AppOverflowMenu(
                hasIdentity: snapshot.identity != null,
                onSelected: (action) => _handleAppAction(action, snapshot),
              ),
            ],
          ),
          body: snapshot.identity == null
              ? _IdentitySetup(gateway: widget.gateway)
              : LayoutBuilder(builder: (context, constraints) {
                  void contactInfo(ContactDto contact) => _openContactDetails(contact);
                  void conversationAction(
                    ConversationDto conversation,
                    ContactDto contact,
                    ConversationAction action,
                  ) =>
                      _handleConversationAction(conversation, contact, action);
                  if (constraints.maxWidth < _wideLayoutBreakpoint) {
                    return _ConversationList(
                      conversations: snapshot.conversations,
                      contacts: snapshot.contacts,
                      selectedConversationId: null,
                      onContactInfo: contactInfo,
                      onAction: conversationAction,
                      onSelected: (conversation) => Navigator.of(context).push<void>(
                        MaterialPageRoute(
                          builder: (_) => ConversationScreen(
                            gateway: widget.gateway,
                            conversation: conversation,
                          ),
                        ),
                      ),
                    );
                  }
                  final selected = _selectedConversation(snapshot.conversations);
                  return Row(children: <Widget>[
                    SizedBox(
                      width: _conversationRailWidth,
                      child: _ConversationList(
                        conversations: snapshot.conversations,
                        contacts: snapshot.contacts,
                        selectedConversationId: selected?.id,
                        onContactInfo: contactInfo,
                        onAction: conversationAction,
                        onSelected: (conversation) =>
                            setState(() => _selectedConversationId = conversation.id),
                      ),
                    ),
                    const VerticalDivider(width: 1),
                    Expanded(
                      child: selected == null
                          ? const _ConversationPlaceholder()
                          : ConversationPane(
                              key: ValueKey(selected.id),
                              gateway: widget.gateway,
                              conversation: selected,
                            ),
                    ),
                  ]);
                }),
          floatingActionButton: snapshot.identity == null
              ? null
              : FloatingActionButton(
                  tooltip: 'Pair contact',
                  onPressed: _openPairing,
                  child: const Icon(Icons.person_add_alt_1),
                ),
        ),
      );

  void _handleAppAction(AppOverflowAction action, AppSnapshotDto snapshot) {
    switch (action) {
      case AppOverflowAction.pairing:
        _openPairing();
      case AppOverflowAction.identity:
        Navigator.of(context).push<void>(
          MaterialPageRoute(builder: (_) => IdentityDetailsScreen(snapshot: snapshot)),
        );
      case AppOverflowAction.diagnostics:
        Navigator.of(context).push<void>(
          MaterialPageRoute(builder: (_) => DiagnosticsScreen(gateway: widget.gateway)),
        );
      case AppOverflowAction.settings:
        Navigator.of(context).push<void>(
          MaterialPageRoute(builder: (_) => SettingsScreen(preferences: widget.preferences)),
        );
      case AppOverflowAction.about:
        showAboutDialog(
          context: context,
          applicationName: 'Torca',
          applicationVersion: '0.1 alpha',
          applicationLegalese: 'Private 1:1 messaging over Tor.',
        );
    }
  }

  Future<void> _handleConversationAction(
    ConversationDto conversation,
    ContactDto contact,
    ConversationAction action,
  ) async {
    switch (action) {
      case ConversationAction.open:
        return;
      case ConversationAction.contactDetails:
        _openContactDetails(contact);
      case ConversationAction.rename:
        await _renameContact(contact);
      case ConversationAction.clearHistory:
        if (!await _confirm(
          'Clear conversation history?',
          'Messages, receipts, pending delivery work and local encrypted attachment files for this conversation will be deleted.',
          'Clear history',
        )) return;
        await _execute(
          ClearConversationHistoryCommandDto(conversationIdHex: conversation.id),
          'Could not clear conversation history',
        );
      case ConversationAction.blockToggle:
        if (contact.status == 'blocked') {
          await _execute(
            UnblockContactCommandDto(contactIdHex: contact.id),
            'Could not unblock contact',
          );
        } else {
          if (!await _confirm(
            'Block ${contact.displayName}?',
            'The current peer connection will be closed and Torca will not reconnect until you unblock this contact.',
            'Block',
          )) return;
          await _execute(
            BlockContactCommandDto(contactIdHex: contact.id),
            'Could not block contact',
          );
        }
      case ConversationAction.remove:
        if (!await _confirm(
          'Remove ${contact.displayName}?',
          'This removes the contact, local conversation history, pending work and protected peer credential. This cannot be undone.',
          'Remove',
        )) return;
        await _execute(
          RemoveContactCommandDto(contactIdHex: contact.id),
          'Could not remove contact',
        );
    }
  }

  Future<void> _renameContact(ContactDto contact) async {
    final controller = TextEditingController(text: contact.displayName);
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Rename contact'),
        content: TextField(
          controller: controller,
          autofocus: true,
          maxLength: 64,
          decoration: const InputDecoration(labelText: 'Local name'),
          onSubmitted: (value) => Navigator.of(context).pop(value),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(controller.text),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    controller.dispose();
    final normalized = name?.trim();
    if (normalized == null || normalized.isEmpty || !mounted) return;
    await _execute(
      RenameContactCommandDto(contactIdHex: contact.id, displayName: normalized),
      'Could not rename contact',
    );
  }

  Future<bool> _confirm(String title, String message, String action) async =>
      await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(title),
          content: Text(message),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(action),
            ),
          ],
        ),
      ) ??
      false;

  Future<void> _execute(BridgeCommandDto command, String fallbackError) async {
    final result = await widget.gateway.execute(command);
    if (mounted && !result.ok) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(result.error ?? fallbackError)),
      );
    }
  }

  void _openContactDetails(ContactDto contact) => Navigator.of(context).push<void>(
        MaterialPageRoute(
          builder: (_) => ContactDetailsScreen(gateway: widget.gateway, contact: contact),
        ),
      );

  void _openPairing() => Navigator.of(context).push<void>(
        MaterialPageRoute(builder: (_) => PairingScreen(gateway: widget.gateway)),
      );

  ConversationDto? _selectedConversation(List<ConversationDto> conversations) {
    if (conversations.isEmpty) return null;
    final selectedId = _selectedConversationId;
    if (selectedId != null) {
      for (final conversation in conversations) {
        if (conversation.id == selectedId) return conversation;
      }
    }
    return conversations.first;
  }
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
      return const Center(
        child: Padding(
          padding: EdgeInsets.all(24),
          child: Text('Pair a contact to start a conversation.'),
        ),
      );
    }
    return ListView.builder(
      itemCount: conversations.length,
      itemBuilder: (context, index) {
        final conversation = conversations[index];
        final contact = _contact(conversation.contactId);
        final tile = ListTile(
          selected: conversation.id == selectedConversationId,
          leading: const CircleAvatar(child: Icon(Icons.person_outline)),
          title: Text(contact?.displayName ?? 'Contact'),
          subtitle: Text(contact?.status == 'blocked' ? 'Blocked' : conversation.status),
          trailing: Row(mainAxisSize: MainAxisSize.min, children: <Widget>[
            ConnectionIndicator(
              state: contact?.connectionState ?? 'disconnected',
              blocked: contact?.status == 'blocked',
            ),
            if (contact != null)
              IconButton(
                tooltip: 'Contact details',
                icon: const Icon(Icons.info_outline, size: 19),
                onPressed: () => onContactInfo(contact),
              ),
          ]),
          onTap: () => onSelected(conversation),
          onLongPress: contact == null
              ? null
              : () => _showActions(context, conversation, contact),
        );
        return GestureDetector(
          behavior: HitTestBehavior.opaque,
          onSecondaryTapDown: contact == null
              ? null
              : (details) => _showActions(
                    context,
                    conversation,
                    contact,
                    globalPosition: details.globalPosition,
                  ),
          child: tile,
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
    final blocked = contact.status == 'blocked';
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

class _ConversationPlaceholder extends StatelessWidget {
  const _ConversationPlaceholder();
  @override
  Widget build(BuildContext context) => const Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(Icons.forum_outlined, size: 48),
            SizedBox(height: 12),
            Text('Select a conversation'),
          ],
        ),
      );
}

class _IdentitySetup extends StatefulWidget {
  const _IdentitySetup({required this.gateway});
  final EngineGateway gateway;
  @override
  State<_IdentitySetup> createState() => _IdentitySetupState();
}

class _IdentitySetupState extends State<_IdentitySetup> {
  final TextEditingController controller = TextEditingController();
  final Random _random = Random.secure();
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
                  'Create local identity',
                  style: Theme.of(context).textTheme.headlineSmall,
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 20),
                TextField(
                  controller: controller,
                  enabled: !_submitting,
                  decoration: InputDecoration(labelText: 'Display name', errorText: _error),
                  onSubmitted: _submitting ? null : (_) => _createIdentity(),
                ),
                const SizedBox(height: 12),
                FilledButton(
                  onPressed: _submitting ? null : _createIdentity,
                  child: Text(_submitting ? 'Creating…' : 'Create local identity'),
                ),
              ],
            ),
          ),
        ),
      );

  Future<void> _createIdentity() async {
    final displayName = controller.text.trim();
    if (displayName.isEmpty) {
      setState(() => _error = 'Display name is required');
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    final result = await widget.gateway.execute(CreateIdentityCommandDto(
      identityIdHex: _newId(),
      displayName: displayName,
      atMs: DateTime.now().millisecondsSinceEpoch,
    ));
    if (!mounted) return;
    setState(() {
      _submitting = false;
      _error = result.ok ? null : result.error ?? 'Could not create local identity';
    });
  }

  String _newId() {
    final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
    if (bytes.every((value) => value == 0)) bytes[15] = 1;
    return bytes.map((value) => value.toRadixString(16).padLeft(2, '0')).join();
  }
}
