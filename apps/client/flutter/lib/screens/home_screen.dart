import 'dart:async';

import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../settings/local_preferences.dart';
import '../widgets/adaptive_app_shell.dart';
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

const double _wideLayoutBreakpoint = 960;
const double _conversationRailWidth = 360;

enum _HomeSection { chats, contacts, invitations }

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

class _BootstrapProgressScreen extends StatefulWidget {
  const _BootstrapProgressScreen({required this.snapshot, this.onRetry});

  final AppSnapshotDto snapshot;
  final VoidCallback? onRetry;

  @override
  State<_BootstrapProgressScreen> createState() =>
      _BootstrapProgressScreenState();
}

class _BootstrapProgressScreenState extends State<_BootstrapProgressScreen> {
  late final DateTime _startedAt = DateTime.now();
  late final Timer _clock = Timer.periodic(const Duration(seconds: 1), (_) {
    if (mounted) setState(() {});
  });

  Duration get _elapsed => DateTime.now().difference(_startedAt);

  @override
  void dispose() {
    _clock.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme;
    const steps = <String>[
      'local_storage',
      'device_identity',
      'tor_network',
      'onion_service',
      'secure_relay',
    ];
    final ready = steps
        .where((id) => _stateFor(widget.snapshot, id) == 'ready')
        .length;
    return Scaffold(
      body: DecoratedBox(
        decoration: BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: <Color>[color.primaryContainer, color.surface],
          ),
        ),
        child: SafeArea(
          child: SingleChildScrollView(
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 620),
                child: Padding(
                  padding: const EdgeInsets.all(28),
                  child: Card(
                    elevation: 0,
                    color: color.surface.withValues(alpha: 0.92),
                    child: Padding(
                      padding: const EdgeInsets.all(24),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          CircleAvatar(
                            radius: 30,
                            backgroundColor: color.primaryContainer,
                            foregroundColor: color.onPrimaryContainer,
                            child: const Icon(Icons.shield_outlined, size: 32),
                          ),
                          const SizedBox(height: 16),
                          Text(
                            'Preparing your private space',
                            style: Theme.of(context).textTheme.headlineSmall,
                            textAlign: TextAlign.center,
                          ),
                          const SizedBox(height: 8),
                          Text(
                            'Setting up encrypted storage and a private Tor connection. You can safely leave this screen open.',
                            style: Theme.of(context).textTheme.bodyMedium,
                            textAlign: TextAlign.center,
                          ),
                          const SizedBox(height: 22),
                          ClipRRect(
                            borderRadius: BorderRadius.circular(99),
                            child: LinearProgressIndicator(
                              value: ready / steps.length,
                            ),
                          ),
                          const SizedBox(height: 8),
                          Text(
                            '$ready of ${steps.length} secure checks complete  •  ${_formatDuration(_elapsed)}',
                          ),
                          const SizedBox(height: 16),
                          for (final id in steps)
                            _BootstrapStepTile(
                              id: id,
                              label: _bootstrapLabel(id),
                              state: _stateFor(widget.snapshot, id),
                              code: _codeFor(widget.snapshot, id),
                              elapsed: _elapsed,
                            ),
                          if (widget.snapshot.bootstrapPhase == 'failed' ||
                              widget.snapshot.bootstrapPhase ==
                                  'degraded') ...<Widget>[
                            const SizedBox(height: 12),
                            Text(
                              _diagnostic(widget.snapshot),
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
                                  onPressed: widget.onRetry,
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
            ),
          ),
        ),
      ),
    );
  }

  String _stateFor(AppSnapshotDto snapshot, String id) {
    return _stateForId(snapshot, id);
  }

  String _stateForId(AppSnapshotDto snapshot, String id) {
    final match = snapshot.bootstrapSteps.where((step) => step.id == id);
    return match.isEmpty ? 'pending' : match.first.state;
  }

  String? _codeFor(AppSnapshotDto snapshot, String id) {
    final match = snapshot.bootstrapSteps.where((step) => step.id == id);
    return match.isEmpty ? null : match.first.code;
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

  String _formatDuration(Duration value) {
    final minutes = value.inMinutes.toString().padLeft(2, '0');
    final seconds = (value.inSeconds % 60).toString().padLeft(2, '0');
    return '$minutes:$seconds';
  }
}

class _BootstrapStepTile extends StatelessWidget {
  const _BootstrapStepTile({
    required this.id,
    required this.label,
    required this.state,
    this.code,
    required this.elapsed,
  });
  final String id;
  final String label;
  final String state;
  final String? code;
  final Duration elapsed;

  @override
  Widget build(BuildContext context) {
    final ready = state == 'ready';
    final running =
        state == 'running' || state == 'verifying' || state == 'retrying';
    final degraded = state == 'degraded';
    return AnimatedContainer(
      duration: const Duration(milliseconds: 250),
      margin: const EdgeInsets.symmetric(vertical: 3),
      decoration: BoxDecoration(
        color: ready
            ? Theme.of(
                context,
              ).colorScheme.primaryContainer.withValues(alpha: 0.45)
            : null,
        borderRadius: BorderRadius.circular(12),
      ),
      child: ListTile(
        dense: true,
        leading: Icon(
          ready
              ? Icons.check_circle
              : degraded
              ? Icons.cloud_off_outlined
              : running
              ? Icons.sync_outlined
              : Icons.radio_button_unchecked,
          color: ready
              ? Colors.green
              : degraded
              ? Theme.of(context).colorScheme.tertiary
              : running
              ? Theme.of(context).colorScheme.primary
              : null,
        ),
        title: Text(label),
        subtitle: Text(_stateDescription(id, state, code, elapsed)),
        trailing: running
            ? Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Text(_formatDuration(elapsed)),
                  const SizedBox(width: 10),
                  const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                ],
              )
            : null,
      ),
    );
  }

  String _stateDescription(
    String id,
    String value,
    String? code,
    Duration elapsed,
  ) {
    if (value == 'running' || value == 'verifying') {
      if (id == 'tor_network') {
        return switch (code) {
          'TOR_CONNECTING_DIRECTORY' =>
            'Opening secure channels to the Tor directory…',
          'TOR_DOWNLOADING_CONSENSUS' =>
            'Downloading network consensus and selecting guards…',
          'TOR_BUILDING_CIRCUITS' => 'Building the first private Tor circuits…',
          'TOR_BOOTSTRAP_SLOW' =>
            'Tor is taking longer than usual; the watchdog is monitoring progress…',
          _ => 'Preparing the embedded Tor client…',
        };
      }
      return switch (id) {
        'local_storage' => 'Opening encrypted storage and checking its schema…',
        'device_identity' => 'Loading device keys and calculating fingerprint…',
        'onion_service' => 'Publishing this device’s private onion service…',
        'secure_relay' => 'Testing the embedded relay endpoint through Tor…',
        _ => 'Working securely…',
      };
    }
    return switch (value) {
      'ready' => switch (id) {
        'local_storage' => 'Encrypted database is open',
        'device_identity' => 'Device identity is protected and ready',
        'tor_network' => 'Tor circuits are available',
        'onion_service' => 'Private onion service is published',
        'secure_relay' => 'Secure relay is reachable',
        _ => 'Protected and ready',
      },
      'retrying' => 'Retrying in the background…',
      'degraded' => 'Temporarily unavailable; retrying',
      'failed' => 'Needs attention',
      _ => 'Waiting for the previous secure check',
    };
  }

  String _formatDuration(Duration value) {
    final minutes = value.inMinutes.toString().padLeft(2, '0');
    final seconds = (value.inSeconds % 60).toString().padLeft(2, '0');
    return '$minutes:$seconds';
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
  String? _selectedContactId;
  _HomeSection _section = _HomeSection.chats;

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
      final profileMissing =
          snapshot.identity == null || snapshot.identity!.displayName == null;
      return AdaptiveAppShell(
        title: snapshot.identity?.displayName ?? 'Torca',
        selectedIndex: _section.index,
        onDestinationSelected: (index) =>
            setState(() => _section = _HomeSection.values[index]),
        destinations: const <NavigationDestination>[
          NavigationDestination(
            icon: Icon(Icons.forum_outlined),
            selectedIcon: Icon(Icons.forum),
            label: 'Chats',
          ),
          NavigationDestination(
            icon: Icon(Icons.people_outline),
            selectedIcon: Icon(Icons.people),
            label: 'Contacts',
          ),
          NavigationDestination(
            icon: Icon(Icons.qr_code_2_outlined),
            selectedIcon: Icon(Icons.qr_code_2),
            label: 'Invitations',
          ),
        ],
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
        body: profileMissing
            ? _ProfileSetup(
                gateway: widget.gateway,
                fingerprint: snapshot.identity?.fingerprint,
              )
            : _sectionBody(snapshot),
        floatingActionButton: profileMissing
            ? null
            : FloatingActionButton(
                tooltip: 'Pair contact',
                onPressed: _openPairing,
                child: const Icon(Icons.person_add_alt_1),
              ),
      );
    },
  );

  Widget _sectionBody(AppSnapshotDto snapshot) => switch (_section) {
    _HomeSection.chats => _chats(snapshot),
    _HomeSection.contacts => _ContactsSection(
      contacts: snapshot.contacts,
      selectedContactId: _selectedContactId,
      onSelected: (contact) => setState(() => _selectedContactId = contact.id),
      onOpenDetails: _openContactDetails,
    ),
    _HomeSection.invitations => _InvitationsSection(
      pairings: snapshot.pairings,
      onOpen: _openPairing,
    ),
  };

  Widget _chats(AppSnapshotDto snapshot) => LayoutBuilder(
    builder: (context, constraints) {
      final conversations = snapshot.conversations;
      if (constraints.maxWidth < _wideLayoutBreakpoint) {
        return _ConversationList(
          conversations: conversations,
          contacts: snapshot.contacts,
          selectedConversationId: null,
          onContactInfo: _openContactDetails,
          onAction: _handleConversationAction,
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
      final selected = _selectedConversation(conversations);
      final contact = selected == null
          ? null
          : _contactFor(snapshot.contacts, selected.contactId);
      final contextPanel = constraints.maxWidth >= 1440 && contact != null;
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
          if (contextPanel) ...<Widget>[
            const VerticalDivider(width: 1),
            SizedBox(
              width: 300,
              child: _ContactContextPanel(
                contact: contact,
                onOpen: () => _openContactDetails(contact),
              ),
            ),
          ],
        ],
      );
    },
  );

  ContactDto? _contactFor(List<ContactDto> contacts, String id) {
    for (final contact in contacts) {
      if (contact.id == id) return contact;
    }
    return null;
  }

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

class _ContactsSection extends StatelessWidget {
  const _ContactsSection({
    required this.contacts,
    required this.selectedContactId,
    required this.onSelected,
    required this.onOpenDetails,
  });

  final List<ContactDto> contacts;
  final String? selectedContactId;
  final ValueChanged<ContactDto> onSelected;
  final ValueChanged<ContactDto> onOpenDetails;

  @override
  Widget build(BuildContext context) {
    if (contacts.isEmpty) {
      return const _SectionEmptyState(
        icon: Icons.people_outline,
        title: 'No contacts yet',
        message: 'Create an invitation to add a private contact.',
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
            Text('Contacts', style: Theme.of(context).textTheme.headlineSmall),
            const SizedBox(height: 8),
            Text(
              '${contacts.length} private ${contacts.length == 1 ? 'contact' : 'contacts'}',
            ),
            const SizedBox(height: 12),
            for (final contact in contacts)
              Card(
                clipBehavior: Clip.antiAlias,
                child: ListTile(
                  selected: wide && contact.id == active.id,
                  onTap: () =>
                      wide ? onSelected(contact) : onOpenDetails(contact),
                  leading: CircleAvatar(
                    child: Text(_initial(contact.displayName)),
                  ),
                  title: Text(contact.displayName),
                  subtitle: Text(
                    '${contact.verificationStatus} · ${contact.connectionState}',
                  ),
                  trailing: const Icon(Icons.chevron_right),
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
                onOpen: () => onOpenDetails(active),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _InvitationsSection extends StatelessWidget {
  const _InvitationsSection({required this.pairings, required this.onOpen});

  final List<PairingDto> pairings;
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) => ListView(
    padding: const EdgeInsets.all(24),
    children: <Widget>[
      Text('Invitations', style: Theme.of(context).textTheme.headlineSmall),
      const SizedBox(height: 8),
      const Text('Create and manage short-lived private contact invitations.'),
      const SizedBox(height: 20),
      FilledButton.icon(
        onPressed: onOpen,
        icon: const Icon(Icons.add_link),
        label: const Text('Create or join invitation'),
      ),
      const SizedBox(height: 24),
      if (pairings.isEmpty)
        const _SectionEmptyState(
          icon: Icons.qr_code_2_outlined,
          title: 'No invitations',
          message:
              'Your active invitations and pairing requests will appear here.',
        )
      else ...<Widget>[
        Text('Recent sessions', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 8),
        for (final pairing in pairings.reversed)
          Card(
            child: ListTile(
              leading: Icon(
                pairing.role == 'creator' ? Icons.qr_code_2 : Icons.link,
              ),
              title: Text(
                pairing.role == 'creator'
                    ? 'Created invitation'
                    : 'Joined invitation',
              ),
              subtitle: Text('Code ${pairing.code}'),
              trailing: Chip(label: Text(pairing.state)),
              onTap: onOpen,
            ),
          ),
      ],
    ],
  );
}

class _ContactContextPanel extends StatelessWidget {
  const _ContactContextPanel({required this.contact, required this.onOpen});

  final ContactDto contact;
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.all(20),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        CircleAvatar(radius: 28, child: Text(_initial(contact.displayName))),
        const SizedBox(height: 14),
        Text(
          contact.displayName,
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 4),
        Text(contact.connectionState),
        const SizedBox(height: 20),
        const Text('Security', style: TextStyle(fontWeight: FontWeight.w600)),
        const SizedBox(height: 6),
        Text('Verification: ${contact.verificationStatus}'),
        Text('Peer health: ${contact.peerHealth.quality}'),
        const Spacer(),
        OutlinedButton.icon(
          onPressed: onOpen,
          icon: const Icon(Icons.person_outline),
          label: const Text('Contact details'),
        ),
      ],
    ),
  );
}

String _initial(String value) => value.isEmpty ? '?' : value[0].toUpperCase();

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
