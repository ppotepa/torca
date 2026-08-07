import 'dart:math';

import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import 'contact_details_screen.dart';
import 'conversation_screen.dart';
import 'diagnostics_screen.dart';
import 'pairing_screen.dart';

const double _wideLayoutBreakpoint = 900;
const double _conversationRailWidth = 360;

class HomeScreen extends StatefulWidget {
  const HomeScreen({required this.gateway, super.key});
  final EngineGateway gateway;
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
            child: _TorStatus(state: snapshot.torState),
          ),
          if (snapshot.identity != null)
            IconButton(
              tooltip: 'Your Torca identity',
              icon: const Icon(Icons.shield_outlined),
              onPressed: () => Navigator.of(context).push<void>(
                MaterialPageRoute(builder: (_) => IdentityDetailsScreen(snapshot: snapshot)),
              ),
            ),
          IconButton(
            tooltip: 'Diagnostics',
            icon: const Icon(Icons.monitor_heart_outlined),
            onPressed: () => Navigator.of(context).push<void>(
              MaterialPageRoute(builder: (_) => DiagnosticsScreen(gateway: widget.gateway)),
            ),
          ),
        ],
      ),
      body: snapshot.identity == null
          ? _IdentitySetup(gateway: widget.gateway)
          : LayoutBuilder(builder: (context, constraints) {
              void contactInfo(ContactDto contact) => Navigator.of(context).push<void>(
                    MaterialPageRoute(builder: (_) => ContactDetailsScreen(contact: contact)),
                  );
              if (constraints.maxWidth < _wideLayoutBreakpoint) {
                return _ConversationList(
                  conversations: snapshot.conversations,
                  contacts: snapshot.contacts,
                  selectedConversationId: null,
                  onContactInfo: contactInfo,
                  onSelected: (conversation) => Navigator.of(context).push<void>(
                    MaterialPageRoute(builder: (_) => ConversationScreen(
                      gateway: widget.gateway,
                      conversation: conversation,
                    )),
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
                    onSelected: (conversation) => setState(() => _selectedConversationId = conversation.id),
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
              onPressed: () => Navigator.of(context).push<void>(
                MaterialPageRoute(builder: (_) => PairingScreen(gateway: widget.gateway)),
              ),
              child: const Icon(Icons.person_add_alt_1),
            ),
    ),
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

class _TorStatus extends StatelessWidget {
  const _TorStatus({required this.state});
  final String state;
  @override
  Widget build(BuildContext context) {
    final ready = state == 'ready';
    return Tooltip(
      message: 'Tor: $state',
      child: Chip(
        avatar: Icon(ready ? Icons.security : Icons.security_outlined, size: 17),
        label: Text(ready ? 'Tor' : state),
        visualDensity: VisualDensity.compact,
      ),
    );
  }
}

class _ConversationList extends StatelessWidget {
  const _ConversationList({
    required this.conversations,
    required this.contacts,
    required this.selectedConversationId,
    required this.onSelected,
    required this.onContactInfo,
  });
  final List<ConversationDto> conversations;
  final List<ContactDto> contacts;
  final String? selectedConversationId;
  final ValueChanged<ConversationDto> onSelected;
  final ValueChanged<ContactDto> onContactInfo;

  @override
  Widget build(BuildContext context) {
    if (conversations.isEmpty) {
      return const Center(child: Padding(
        padding: EdgeInsets.all(24),
        child: Text('Pair a contact to start a conversation.'),
      ));
    }
    return ListView.builder(
      itemCount: conversations.length,
      itemBuilder: (context, index) {
        final conversation = conversations[index];
        final contact = _contact(conversation.contactId);
        final connection = contact?.connectionState ?? 'disconnected';
        return ListTile(
          selected: conversation.id == selectedConversationId,
          leading: const CircleAvatar(child: Icon(Icons.person_outline)),
          title: Text('Contact ${_shortId(conversation.contactId)}'),
          subtitle: Text(conversation.status),
          trailing: Row(mainAxisSize: MainAxisSize.min, children: <Widget>[
            _ConnectionIndicator(state: connection),
            if (contact != null)
              IconButton(
                tooltip: 'Contact details',
                icon: const Icon(Icons.info_outline, size: 19),
                onPressed: () => onContactInfo(contact),
              ),
          ]),
          onTap: () => onSelected(conversation),
        );
      },
    );
  }

  ContactDto? _contact(String id) {
    for (final contact in contacts) {
      if (contact.id == id) return contact;
    }
    return null;
  }
}

class _ConnectionIndicator extends StatelessWidget {
  const _ConnectionIndicator({required this.state});
  final String state;
  @override
  Widget build(BuildContext context) {
    final ready = state == 'ready';
    final connecting = state == 'connecting' || state == 'handshaking' || state == 'reconnecting';
    return Tooltip(
      message: ready ? 'Direct P2P over Tor' : state,
      child: Row(mainAxisSize: MainAxisSize.min, children: <Widget>[
        Icon(ready ? Icons.hub : connecting ? Icons.sync : Icons.cloud_off_outlined, size: 18),
        const SizedBox(width: 5),
        Text(ready ? 'P2P' : connecting ? '…' : 'offline'),
      ]),
    );
  }
}

class _ConversationPlaceholder extends StatelessWidget {
  const _ConversationPlaceholder();
  @override
  Widget build(BuildContext context) => const Center(child: Column(
    mainAxisSize: MainAxisSize.min,
    children: <Widget>[Icon(Icons.forum_outlined, size: 48), SizedBox(height: 12), Text('Select a conversation')],
  ));
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
  void dispose() { controller.dispose(); super.dispose(); }

  @override
  Widget build(BuildContext context) => Center(child: SingleChildScrollView(
    padding: const EdgeInsets.all(24),
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 420),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: <Widget>[
        Text('Create local identity', style: Theme.of(context).textTheme.headlineSmall, textAlign: TextAlign.center),
        const SizedBox(height: 20),
        TextField(
          controller: controller,
          enabled: !_submitting,
          decoration: InputDecoration(labelText: 'Display name', errorText: _error, border: const OutlineInputBorder()),
          onSubmitted: _submitting ? null : (_) => _createIdentity(),
        ),
        const SizedBox(height: 12),
        FilledButton(onPressed: _submitting ? null : _createIdentity, child: Text(_submitting ? 'Creating…' : 'Create local identity')),
      ]),
    ),
  ));

  Future<void> _createIdentity() async {
    final displayName = controller.text.trim();
    if (displayName.isEmpty) { setState(() => _error = 'Display name is required'); return; }
    setState(() { _submitting = true; _error = null; });
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

String _shortId(String value) => value.length <= 8 ? value : value.substring(0, 8);
