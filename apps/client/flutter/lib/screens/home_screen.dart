import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
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
  Widget build(BuildContext context) {
    return ValueListenableBuilder<AppSnapshotDto>(
      valueListenable: widget.gateway.snapshots,
      builder: (BuildContext context, AppSnapshotDto snapshot, Widget? child) {
        return Scaffold(
          appBar: AppBar(
            title: Text(snapshot.identity?.displayName ?? 'Torca'),
            actions: <Widget>[
              IconButton(
                tooltip: 'Diagnostics',
                icon: const Icon(Icons.monitor_heart_outlined),
                onPressed: () => Navigator.of(context).push<void>(
                  MaterialPageRoute<void>(
                    builder: (BuildContext context) => const DiagnosticsScreen(),
                  ),
                ),
              ),
            ],
          ),
          body: snapshot.identity == null
              ? _IdentitySetup(gateway: widget.gateway)
              : LayoutBuilder(
                  builder: (BuildContext context, BoxConstraints constraints) {
                    if (constraints.maxWidth < _wideLayoutBreakpoint) {
                      return _ConversationList(
                        conversations: snapshot.conversations,
                        selectedConversationId: null,
                        onSelected: (ConversationDto conversation) {
                          Navigator.of(context).push<void>(
                            MaterialPageRoute<void>(
                              builder: (BuildContext context) => ConversationScreen(
                                gateway: widget.gateway,
                                conversation: conversation,
                              ),
                            ),
                          );
                        },
                      );
                    }

                    final ConversationDto? selected =
                        _selectedConversation(snapshot.conversations);
                    return Row(
                      children: <Widget>[
                        SizedBox(
                          width: _conversationRailWidth,
                          child: _ConversationList(
                            conversations: snapshot.conversations,
                            selectedConversationId: selected?.id,
                            onSelected: (ConversationDto conversation) {
                              setState(() {
                                _selectedConversationId = conversation.id;
                              });
                            },
                          ),
                        ),
                        const VerticalDivider(width: 1),
                        Expanded(
                          child: selected == null
                              ? const _ConversationPlaceholder()
                              : ConversationPane(
                                  key: ValueKey<String>(selected.id),
                                  gateway: widget.gateway,
                                  conversation: selected,
                                ),
                        ),
                      ],
                    );
                  },
                ),
          floatingActionButton: snapshot.identity == null
              ? null
              : FloatingActionButton(
                  tooltip: 'Pair contact',
                  onPressed: () => Navigator.of(context).push<void>(
                    MaterialPageRoute<void>(
                      builder: (BuildContext context) => PairingScreen(
                        gateway: widget.gateway,
                      ),
                    ),
                  ),
                  child: const Icon(Icons.person_add_alt_1),
                ),
        );
      },
    );
  }

  ConversationDto? _selectedConversation(List<ConversationDto> conversations) {
    if (conversations.isEmpty) {
      return null;
    }
    final String? selectedId = _selectedConversationId;
    if (selectedId != null) {
      for (final ConversationDto conversation in conversations) {
        if (conversation.id == selectedId) {
          return conversation;
        }
      }
    }
    return conversations.first;
  }
}

class _ConversationList extends StatelessWidget {
  const _ConversationList({
    required this.conversations,
    required this.selectedConversationId,
    required this.onSelected,
  });

  final List<ConversationDto> conversations;
  final String? selectedConversationId;
  final ValueChanged<ConversationDto> onSelected;

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
      itemBuilder: (BuildContext context, int index) {
        final ConversationDto conversation = conversations[index];
        return ListTile(
          selected: conversation.id == selectedConversationId,
          leading: const CircleAvatar(child: Icon(Icons.person_outline)),
          title: Text('Contact ${_shortId(conversation.contactId)}'),
          subtitle: Text(conversation.status),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => onSelected(conversation),
        );
      },
    );
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
              mainAxisSize: MainAxisSize.min,
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
                  decoration: InputDecoration(
                    labelText: 'Display name',
                    errorText: _error,
                    border: const OutlineInputBorder(),
                  ),
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
    final String displayName = controller.text.trim();
    if (displayName.isEmpty) {
      setState(() {
        _error = 'Display name is required';
      });
      return;
    }

    setState(() {
      _submitting = true;
      _error = null;
    });
    final BridgeResultDto result = await widget.gateway.execute(
      CreateIdentityCommandDto(
        identityIdHex: DateTime.now()
            .microsecondsSinceEpoch
            .toRadixString(16)
            .padLeft(32, '0')
            .substring(0, 32),
        displayName: displayName,
        atMs: DateTime.now().millisecondsSinceEpoch,
      ),
    );
    if (!mounted) {
      return;
    }
    setState(() {
      _submitting = false;
      _error = result.ok ? null : result.error ?? 'Could not create local identity';
    });
  }
}

String _shortId(String value) {
  if (value.length <= 8) {
    return value;
  }
  return value.substring(0, 8);
}
