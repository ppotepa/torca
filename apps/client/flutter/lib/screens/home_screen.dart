import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../settings/local_preferences.dart';
import '../widgets/app_overflow_menu.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/conversation_actions.dart';
import '../widgets/conversation_summary_tile.dart';
import '../widgets/tor_status_indicator.dart';
import 'contact_details_screen.dart';
import 'conversation_screen.dart';
import 'diagnostics_screen.dart';
import 'pairing_screen.dart';
import 'settings_screen.dart';

const double _wideLayoutBreakpoint = 900;
const double _conversationRailWidth = 360;

class _BootstrapFailureScreen extends StatelessWidget {
  const _BootstrapFailureScreen({required this.reason, this.onRetry});

  final String reason;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: Padding(
            padding: const EdgeInsets.all(28),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Icon(
                  Icons.lock_outline,
                  size: 64,
                  color: Theme.of(context).colorScheme.error,
                ),
                const SizedBox(height: 18),
                Text(
                  'Secure runtime is not ready',
                  style: Theme.of(context).textTheme.headlineSmall,
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 12),
                const Text(
                  'Torca could not prepare the local encrypted runtime. '
                  'Your identity has not been changed.',
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 16),
                Text(
                  reason,
                  textAlign: TextAlign.center,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
                if (onRetry != null) ...<Widget>[
                  const SizedBox(height: 22),
                  FilledButton.icon(
                    onPressed: onRetry,
                    icon: const Icon(Icons.refresh),
                    label: const Text('Retry'),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    ),
  );
}

class _BootstrapProgressScreen extends StatelessWidget {
  const _BootstrapProgressScreen({required this.snapshot, this.onRetry});

  final AppSnapshotDto snapshot;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: Padding(
            padding: const EdgeInsets.all(28),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                const LinearProgressIndicator(),
                const SizedBox(height: 22),
                Text(
                  'Preparing secure network',
                  style: Theme.of(context).textTheme.headlineSmall,
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 18),
                for (final id in const <String>[
                  'local_storage',
                  'device_identity',
                  'tor_network',
                  'onion_service',
                  'secure_relay',
                ])
                  _BootstrapStepTile(
                    label: _bootstrapLabel(id),
                    state: _stateFor(snapshot, id),
                  ),
                if (snapshot.bootstrapPhase == 'failed' ||
                    snapshot.bootstrapPhase == 'degraded') ...<Widget>[
                  const SizedBox(height: 12),
                  Text(
                    _diagnostic(snapshot),
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                  const SizedBox(height: 12),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: <Widget>[
                      FilledButton(
                        onPressed: onRetry,
                        child: const Text('Retry'),
                      ),
                    ],
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    ),
  );

  String _stateFor(AppSnapshotDto snapshot, String id) {
    return _stateForId(snapshot, id);
  }

  String _stateForId(AppSnapshotDto snapshot, String id) {
    final match = snapshot.bootstrapSteps.where((step) => step.id == id);
    return match.isEmpty ? 'pending' : match.first.state;
  }

  String _diagnostic(AppSnapshotDto snapshot) {
    final failed = snapshot.bootstrapSteps.where(
      (step) => step.state == 'failed',
    );
    final step = failed.isEmpty ? null : failed.first;
    if (step == null || step.code == null || step.code!.isEmpty) {
      return 'Secure runtime is not ready. Check diagnostics and retry.';
    }
    return '${step.id}: ${step.code}';
  }

  String _bootstrapLabel(String id) => switch (id) {
    'local_storage' => 'Local storage',
    'device_identity' => 'Device identity',
    'tor_network' => 'Tor network',
    'onion_service' => 'Onion service',
    'secure_relay' => 'Secure relay',
    _ => id,
  };
}

class _BootstrapStepTile extends StatelessWidget {
  const _BootstrapStepTile({required this.label, required this.state});
  final String label;
  final String state;

  @override
  Widget build(BuildContext context) {
    final ready = state == 'ready';
    final running = state == 'running' || state == 'verifying';
    return ListTile(
      dense: true,
      leading: Icon(
        ready
            ? Icons.check_circle
            : running
            ? Icons.sync
            : Icons.radio_button_unchecked,
        color: ready
            ? Colors.green
            : running
            ? Theme.of(context).colorScheme.primary
            : null,
      ),
      title: Text(label),
      subtitle: Text(state),
    );
  }
}

class HomeScreen extends StatefulWidget {
  const HomeScreen({
    required this.gateway,
    required this.preferences,
    this.onRetryBootstrap,
    super.key,
  });
  final EngineGateway gateway;
  final LocalPreferences preferences;
  final VoidCallback? onRetryBootstrap;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  String? _selectedConversationId;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: widget.gateway.snapshots,
    builder: (context, snapshot, _) {
      final availability = widget.gateway is GatewayAvailability
          ? widget.gateway as GatewayAvailability
          : null;
      if (availability != null && !availability.isAvailable) {
        return _BootstrapFailureScreen(
          reason: availability.failureReason ?? 'Secure runtime unavailable',
          onRetry: widget.onRetryBootstrap,
        );
      }
      if (snapshot.bootstrapPhase != 'ready' &&
          snapshot.bootstrapPhase != 'ready_for_profile') {
        return _BootstrapProgressScreen(
          snapshot: snapshot,
          onRetry: () =>
              widget.gateway.execute(const RefreshSnapshotCommandDto()),
        );
      }
      return Scaffold(
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
        body:
            snapshot.identity == null || snapshot.identity!.displayName == null
            ? _ProfileSetup(
                gateway: widget.gateway,
                fingerprint: snapshot.identity?.fingerprint,
              )
            : LayoutBuilder(
                builder: (context, constraints) {
                  final conversations = snapshot.conversations;
                  if (constraints.maxWidth < _wideLayoutBreakpoint) {
                    return _ConversationList(
                      conversations: conversations,
                      contacts: snapshot.contacts,
                      selectedConversationId: null,
                      onContactInfo: _openContactDetails,
                      onAction: _handleConversationAction,
                      onSelected: (conversation) =>
                          Navigator.of(context).push<void>(
                            MaterialPageRoute(
                              builder: (_) => ConversationScreen(
                                gateway: widget.gateway,
                                conversation: conversation,
                              ),
                            ),
                          ),
                    );
                  }
                  final selected = _selectedConversation(conversations);
                  return Row(
                    children: <Widget>[
                      SizedBox(
                        width: _conversationRailWidth,
                        child: _ConversationList(
                          conversations: conversations,
                          contacts: snapshot.contacts,
                          selectedConversationId: selected?.id,
                          onContactInfo: _openContactDetails,
                          onAction: _handleConversationAction,
                          onSelected: (conversation) => setState(
                            () => _selectedConversationId = conversation.id,
                          ),
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
                    ],
                  );
                },
              ),
        floatingActionButton: snapshot.identity == null
            ? null
            : FloatingActionButton(
                tooltip: 'Pair contact',
                onPressed: _openPairing,
                child: const Icon(Icons.person_add_alt_1),
              ),
      );
    },
  );

  void _handleAppAction(AppOverflowAction action, AppSnapshotDto snapshot) {
    switch (action) {
      case AppOverflowAction.pairing:
        _openPairing();
      case AppOverflowAction.identity:
        Navigator.of(context).push<void>(
          MaterialPageRoute(
            builder: (_) => IdentityDetailsScreen(snapshot: snapshot),
          ),
        );
      case AppOverflowAction.diagnostics:
        Navigator.of(context).push<void>(
          MaterialPageRoute(
            builder: (_) => DiagnosticsScreen(gateway: widget.gateway),
          ),
        );
      case AppOverflowAction.settings:
        Navigator.of(context).push<void>(
          MaterialPageRoute(
            builder: (_) => SettingsScreen(preferences: widget.preferences),
          ),
        );
      case AppOverflowAction.about:
        showAboutDialog(
          context: context,
          applicationName: 'Torca',
          applicationVersion: '0.2 alpha',
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
        ))
          return;
        await _execute(
          ClearConversationHistoryCommandDto(
            conversationIdHex: conversation.id,
          ),
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
          ))
            return;
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
        ))
          return;
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
      RenameContactCommandDto(
        contactIdHex: contact.id,
        displayName: normalized,
      ),
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
        SnackBar(
          content: Text(
            BridgeErrorPresenter.message(result, fallback: fallbackError),
          ),
        ),
      );
    }
  }

  void _openContactDetails(ContactDto contact) =>
      Navigator.of(context).push<void>(
        MaterialPageRoute(
          builder: (_) =>
              ContactDetailsScreen(gateway: widget.gateway, contact: contact),
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
                  BridgeErrorPresenter.message(
                    result ?? const BridgeResultDto(ok: false, kind: 'error'),
                    fallback: 'Could not save nickname',
                  );
      });
    }
  }
}
