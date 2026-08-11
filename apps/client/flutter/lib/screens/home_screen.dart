import 'dart:async';

import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../settings/local_preferences.dart';
import '../widgets/adaptive_app_shell.dart';
import '../widgets/app_overflow_menu.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/connection_indicator.dart';
import '../widgets/contact_actions.dart';
import '../widgets/conversation_actions.dart';
import '../widgets/conversation_summary_tile.dart';
import '../widgets/runtime_network_status.dart';
import 'contact_details_screen.dart';
import 'conversation_screen.dart';
import 'diagnostics_screen.dart';
import 'pairing_screen.dart';
import 'settings_screen.dart';

const double _wideLayoutBreakpoint = 960;
const String _buildId = String.fromEnvironment(
  'TORCA_BUILD_ID',
  defaultValue: 'dev',
);

enum _HomeSection { chats, contacts, invitations }

class _BootstrapFailureScreen extends StatelessWidget {
  const _BootstrapFailureScreen({required this.reason, this.onRetry});

  final String reason;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: Column(
        children: <Widget>[
          const Padding(
            padding: EdgeInsets.only(top: 8, right: 12),
            child: RuntimeNetworkHeader(),
          ),
          Expanded(
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 520),
                child: Padding(
                  padding: const EdgeInsets.all(28),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Icon(
                        context.torcaIcons.identity,
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
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ),
                      if (onRetry != null) ...<Widget>[
                        const SizedBox(height: 22),
                        FilledButton.icon(
                          onPressed: onRetry,
                          icon: Icon(context.torcaIcons.retry),
                          label: const Text('Retry'),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
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
    final projectedSteps = steps
        .map((id) => _stepFor(widget.snapshot, id))
        .toList(growable: false);
    final progress =
        projectedSteps.fold<int>(0, (sum, step) => sum + step.progress) /
        (steps.length * 100);
    final active = projectedSteps.where(
      (step) =>
          step.typedState == BootstrapStepState.running ||
          step.typedState == BootstrapStepState.verifying,
    );
    final elapsed = active.isEmpty ? _elapsed : _elapsedFor(active.first);
    final restartRequired = projectedSteps.any(
      (step) =>
          step.code == 'TOR_RESTART_REQUIRED' ||
          step.code == 'ONION_SERVICE_RESTART_REQUIRED',
    );
    return Scaffold(
      body: DecoratedBox(
        decoration: BoxDecoration(color: color.surface),
        child: SafeArea(
          child: Column(
            children: <Widget>[
              const Padding(
                padding: EdgeInsets.only(top: 8, right: 12),
                child: RuntimeNetworkHeader(),
              ),
              Expanded(
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
                                TorcaAvatar(
                                  label: 'Identity',
                                  size: 60,
                                  backgroundColor: color.primaryContainer,
                                  foregroundColor: color.onPrimaryContainer,
                                  child: Icon(
                                    context.torcaIcons.identity,
                                    size: 32,
                                  ),
                                ),
                                const SizedBox(height: 16),
                                Text(
                                  'Preparing your private space',
                                  style: Theme.of(
                                    context,
                                  ).textTheme.headlineSmall,
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
                                  borderRadius: BorderRadius.circular(
                                    context.torcaTokens.radiusLarge,
                                  ),
                                  child: LinearProgressIndicator(
                                    value: progress.clamp(0, 1),
                                  ),
                                ),
                                const SizedBox(height: 8),
                                Text(
                                  '$ready of ${steps.length} secure checks complete  •  ${_formatDuration(elapsed)}',
                                ),
                                const SizedBox(height: 16),
                                for (final step in projectedSteps)
                                  _BootstrapStepTile(
                                    step: step,
                                    label: _bootstrapLabel(step.id),
                                    elapsed: _elapsedFor(step),
                                    retryRemaining: _retryRemaining(step),
                                  ),
                                if (widget.snapshot.bootstrapPhase ==
                                        'failed' ||
                                    widget.snapshot.bootstrapPhase ==
                                        'degraded') ...<Widget>[
                                  const SizedBox(height: 12),
                                  Text(
                                    _diagnostic(widget.snapshot),
                                    textAlign: TextAlign.center,
                                    style: TextStyle(
                                      color:
                                          widget.snapshot.bootstrapPhase ==
                                              'degraded'
                                          ? Theme.of(
                                              context,
                                            ).colorScheme.tertiary
                                          : Theme.of(context).colorScheme.error,
                                    ),
                                  ),
                                  const SizedBox(height: 12),
                                  Row(
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: <Widget>[
                                      FilledButton(
                                        onPressed: restartRequired
                                            ? null
                                            : widget.onRetry,
                                        child: Text(
                                          restartRequired
                                              ? 'Restart application'
                                              : 'Retry',
                                        ),
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
            ],
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

  BootstrapStepDto _stepFor(AppSnapshotDto snapshot, String id) {
    final match = snapshot.bootstrapSteps.where((step) => step.id == id);
    return match.isEmpty
        ? BootstrapStepDto(id: id, state: 'pending')
        : match.first;
  }

  Duration _elapsedFor(BootstrapStepDto step) {
    final startedAtMs = step.startedAtMs;
    if (startedAtMs == null) return _elapsed;
    final elapsed = DateTime.now().difference(
      DateTime.fromMillisecondsSinceEpoch(startedAtMs),
    );
    return elapsed.isNegative ? Duration.zero : elapsed;
  }

  Duration? _retryRemaining(BootstrapStepDto step) {
    final retryAtMs = step.retryAtMs;
    if (retryAtMs == null) return null;
    final remaining = DateTime.fromMillisecondsSinceEpoch(
      retryAtMs,
    ).difference(DateTime.now());
    return remaining.isNegative ? Duration.zero : remaining;
  }

  String _diagnostic(AppSnapshotDto snapshot) {
    final failed = snapshot.bootstrapSteps.where(
      (step) =>
          step.typedState == BootstrapStepState.failed ||
          step.typedState == BootstrapStepState.degraded,
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
    required this.step,
    required this.label,
    required this.elapsed,
    this.retryRemaining,
  });
  final BootstrapStepDto step;
  final String label;
  final Duration elapsed;
  final Duration? retryRemaining;

  @override
  Widget build(BuildContext context) {
    final ready = step.typedState == BootstrapStepState.ready;
    final running =
        step.typedState == BootstrapStepState.running ||
        step.typedState == BootstrapStepState.verifying;
    final degraded = step.typedState == BootstrapStepState.degraded;
    return Container(
      margin: const EdgeInsets.symmetric(vertical: 3),
      decoration: BoxDecoration(
        color: ready
            ? Theme.of(
                context,
              ).colorScheme.primaryContainer.withValues(alpha: 0.45)
            : null,
        borderRadius: BorderRadius.circular(context.torcaTokens.radiusLarge),
      ),
      child: ListTile(
        dense: true,
        leading: Icon(
          ready
              ? context.torcaIcons.success
              : degraded
              ? context.torcaIcons.error
              : running
              ? context.torcaIcons.reconnect
              : context.torcaIcons.queued,
          color: ready
              ? context.torcaColors.connectionReady
              : degraded
              ? Theme.of(context).colorScheme.tertiary
              : running
              ? Theme.of(context).colorScheme.primary
              : null,
        ),
        title: Text(
          step.attempt > 0 &&
                  (step.id == 'tor_network' ||
                      step.id == 'onion_service' ||
                      step.id == 'secure_relay')
              ? '$label  •  attempt ${step.attempt} of 3'
              : label,
        ),
        subtitle: Text(_stateDescription(step, retryRemaining)),
        trailing: running
            ? Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (step.progress > 0) ...<Widget>[
                    Text('${step.progress}%'),
                    const SizedBox(width: 10),
                  ],
                  Text(
                    retryRemaining != null
                        ? _formatDuration(retryRemaining!)
                        : _formatDuration(elapsed),
                  ),
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

  String _stateDescription(BootstrapStepDto step, Duration? retryRemaining) {
    final id = step.id;
    final value = step.state;
    final code = step.code;
    if (value == 'running' || value == 'verifying') {
      if (id == 'tor_network') {
        return switch (code) {
          'TOR_CONNECTING_DIRECTORY' =>
            'Opening secure channels to the Tor directory…',
          'TOR_DIRECTORY_CONSENSUS' =>
            'Channels are ready; waiting for Tor directory consensus…',
          'TOR_BOOTSTRAP_BLOCKED' =>
            'Arti reports that directory bootstrap is blocked…',
          _ => 'Preparing the embedded Tor client…',
        };
      }
      if (id == 'onion_service') {
        return switch (code) {
          'ONION_SERVICE_PUBLISHING' =>
            'Publishing this device’s private onion service…',
          _ => 'Preparing the private onion service…',
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
      'retrying' =>
        retryRemaining == null
            ? 'Preparing a controlled retry…'
            : 'Previous attempt failed; retrying in ${_formatDuration(retryRemaining)}',
      'degraded' => 'Temporarily unavailable; retrying',
      'failed'
          when code == 'TOR_RESTART_REQUIRED' ||
              code == 'ONION_SERVICE_RESTART_REQUIRED' =>
        'Tor did not stop safely; restart the application before retrying',
      'blocked' => 'Waiting for the Tor network to become ready',
      'failed' => 'Needs attention: ${code ?? 'TOR_RUNTIME_FAILED'}',
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
  double _conversationListWidth = 340;
  double _contactPanelWidth = 300;

  Future<void> _showBuildInfo(
    ClientBuildInfo? client,
    RelayInfoDto? relay,
  ) => showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('Build & connection info'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            _buildDetail('Version', client?.productVersion ?? 'development'),
            _buildDetail('Flutter build', _buildId),
            _buildDetail('Rust build', client?.buildId ?? 'unavailable'),
            _buildDetail('Source', client?.sourceFingerprint ?? 'unknown'),
            _buildDetail('Commit', client?.sourceCommit ?? 'unknown'),
            _buildDetail(
              'Target',
              client == null
                  ? 'unknown'
                  : '${client.targetPlatform}/${client.targetArchitecture}',
            ),
            _buildDetail(
              'Contract / wire',
              client == null
                  ? 'unknown'
                  : '${client.contractSchema} / ${client.wireVersion}',
            ),
            _buildDetail(
              'Endpoint hash',
              client?.relayEndpointHash ?? 'unknown',
            ),
            const Divider(),
            _buildDetail(
              'Relay version',
              relay?.productVersion ?? 'unavailable',
            ),
            _buildDetail('Relay build', relay?.buildId ?? 'unavailable'),
            _buildDetail('Relay commit', relay?.sourceCommit ?? 'unavailable'),
            _buildDetail(
              'Relay protocol',
              relay == null ? 'unavailable' : '${relay.protocolVersion}',
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Close'),
        ),
      ],
    ),
  );

  Widget _buildDetail(String label, String value) => Padding(
    padding: const EdgeInsets.only(bottom: 7),
    child: SelectableText('$label: $value'),
  );

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
      if (snapshot.typedBootstrapPhase != BootstrapPhase.ready &&
          snapshot.typedBootstrapPhase != BootstrapPhase.readyForProfile) {
        return _BootstrapProgressScreen(
          snapshot: snapshot,
          onRetry: () =>
              widget.gateway.execute(const RefreshSnapshotCommandDto()),
        );
      }
      final profileMissing =
          snapshot.identity == null || snapshot.identity!.displayName == null;
      final buildInfo = widget.gateway is BuildInfoProvider
          ? (widget.gateway as BuildInfoProvider).buildInfo
          : null;
      final relayInfo = snapshot.transport.relayInfo;
      return AdaptiveAppShell(
        title: snapshot.identity?.displayName ?? 'Torca',
        buildLabel:
            'f ${_shortBuild(_buildId)} / '
            'r ${_shortBuild(buildInfo?.buildId ?? '—')}',
        relayLabel: relayInfo == null
            ? 'rel —'
            : 'rel ${_shortBuild(relayInfo.buildId)}',
        onBuildInfo: () => _showBuildInfo(buildInfo, relayInfo),
        selectedIndex: _section.index,
        onDestinationSelected: (index) {
          final section = _HomeSection.values[index];
          setState(() => _section = section);
          if (section == _HomeSection.contacts) {
            unawaited(
              widget.gateway.execute(const AcknowledgeNewContactsCommandDto()),
            );
          }
        },
        destinations: <NavigationDestination>[
          NavigationDestination(
            icon: _NavigationIcon(
              icon: context.torcaIcons.chats,
              count: snapshot.navigationBadges.unreadMessages,
            ),
            selectedIcon: _NavigationIcon(
              icon: context.torcaIcons.chats,
              count: snapshot.navigationBadges.unreadMessages,
            ),
            label: 'Chats',
          ),
          NavigationDestination(
            icon: _NavigationIcon(
              icon: context.torcaIcons.contacts,
              count: snapshot.navigationBadges.newContacts,
            ),
            selectedIcon: _NavigationIcon(
              icon: context.torcaIcons.contacts,
              count: snapshot.navigationBadges.newContacts,
            ),
            label: 'Contacts',
          ),
          NavigationDestination(
            icon: _NavigationIcon(
              icon: context.torcaIcons.invitations,
              count: snapshot.navigationBadges.pairingAttention,
            ),
            selectedIcon: _NavigationIcon(
              icon: context.torcaIcons.invitations,
              count: snapshot.navigationBadges.pairingAttention,
            ),
            label: 'Invitations',
          ),
        ],
        actions: <Widget>[
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
        floatingActionButton:
            profileMissing ||
                _section == _HomeSection.invitations ||
                (_section == _HomeSection.chats &&
                    _visibleConversations(snapshot).isNotEmpty)
            ? null
            : FloatingActionButton(
                tooltip: 'Pair contact',
                onPressed: _openJoinInvitation,
                child: Icon(context.torcaIcons.addContact),
              ),
      );
    },
  );

  Widget _sectionBody(AppSnapshotDto snapshot) => switch (_section) {
    _HomeSection.chats => _chats(snapshot),
    _HomeSection.contacts => _ContactsSection(
      contacts: snapshot.contacts,
      selectedContactId: _selectedContactId,
      onOpenDetails: _openContactDetails,
      onOpenConversation: _openConversationForContact,
      onAction: _handleContactAction,
    ),
    _HomeSection.invitations => _InvitationsSection(
      pairings: snapshot.pairings,
      onOpen: _openInvitationGenerator,
      onOpenInvitation: (pairing) =>
          showPairingSessionModal(context, widget.gateway, pairing),
    ),
  };

  Widget _chats(AppSnapshotDto snapshot) => LayoutBuilder(
    builder: (context, constraints) {
      final conversations = _visibleConversations(snapshot);
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
      final contextPanel = constraints.maxWidth >= 1200 && contact != null;
      final listWidth = _conversationListWidth.clamp(240.0, 520.0);
      final contactWidth = _contactPanelWidth.clamp(260.0, 480.0);
      return Row(
        children: <Widget>[
          SizedBox(
            width: listWidth,
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
          _PaneDivider(
            onDrag: (delta) => setState(() {
              _conversationListWidth = (_conversationListWidth + delta).clamp(
                240.0,
                520.0,
              );
            }),
          ),
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
            _PaneDivider(
              onDrag: (delta) => setState(() {
                _contactPanelWidth = (_contactPanelWidth - delta).clamp(
                  260.0,
                  480.0,
                );
              }),
            ),
            SizedBox(
              width: contactWidth,
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

  List<ConversationDto> _visibleConversations(AppSnapshotDto snapshot) =>
      snapshot.conversations
          .where(
            (conversation) =>
                conversation.lastMessageBody != null ||
                conversation.id == _selectedConversationId,
          )
          .toList(growable: false);

  ContactDto? _contactFor(List<ContactDto> contacts, String id) {
    for (final contact in contacts) {
      if (contact.id == id) return contact;
    }
    return null;
  }

  void _handleAppAction(AppOverflowAction action, AppSnapshotDto snapshot) {
    switch (action) {
      case AppOverflowAction.pairing:
        _openJoinInvitation();
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
        final buildInfo = widget.gateway is BuildInfoProvider
            ? (widget.gateway as BuildInfoProvider).buildInfo
            : null;
        showAboutDialog(
          context: context,
          applicationName: 'Torca',
          applicationVersion: buildInfo?.productVersion ?? 'development',
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
        await ContactActions.rename(context, widget.gateway, contact);
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
        await ContactActions.toggleBlock(context, widget.gateway, contact);
      case ConversationAction.remove:
        await ContactActions.remove(context, widget.gateway, contact);
    }
  }

  Future<void> _handleContactAction(
    ContactDto contact,
    ContactAction action,
  ) async {
    switch (action) {
      case ContactAction.open:
        _openConversationForContact(contact);
      case ContactAction.contactDetails:
        _openContactDetails(contact);
      case ContactAction.rename:
        await ContactActions.rename(context, widget.gateway, contact);
      case ContactAction.blockToggle:
        await ContactActions.toggleBlock(context, widget.gateway, contact);
      case ContactAction.remove:
        final removed = await ContactActions.remove(
          context,
          widget.gateway,
          contact,
        );
        if (mounted && removed && _selectedContactId == contact.id) {
          setState(() => _selectedContactId = null);
        }
    }
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

  void _openContactDetails(ContactDto contact) {
    if (MediaQuery.sizeOf(context).width >= _wideLayoutBreakpoint) {
      setState(() => _selectedContactId = contact.id);
      return;
    }
    Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (_) => ContactDetailsScreen(
          gateway: widget.gateway,
          contact: contact,
          onStartConversation: () async {
            Navigator.of(context).pop();
            await Future<void>.delayed(Duration.zero);
            if (mounted) _openConversationForContact(contact);
          },
        ),
      ),
    );
  }

  void _openConversationForContact(ContactDto contact) {
    unawaited(_ensureAndOpenConversation(contact));
  }

  Future<void> _ensureAndOpenConversation(ContactDto contact) async {
    ConversationDto? conversation;
    for (final candidate in widget.gateway.snapshots.value.conversations) {
      if (candidate.contactId == contact.id) {
        conversation = candidate;
        break;
      }
    }
    if (conversation == null) {
      final result = await widget.gateway.execute(
        StartConversationCommandDto(contactIdHex: contact.id),
      );
      if (!result.ok) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(
                'Could not start conversation with ${contact.displayName}.',
              ),
            ),
          );
        }
        return;
      }
      final conversationId = result.resourceId;
      if (conversationId == null || conversationId.isEmpty) return;
      conversation = ConversationDto(
        id: conversationId,
        contactId: contact.id,
        status: 'active',
      );
    }
    if (MediaQuery.sizeOf(context).width < _wideLayoutBreakpoint) {
      Navigator.of(context).push<void>(
        MaterialPageRoute(
          builder: (_) => ConversationScreen(
            gateway: widget.gateway,
            conversation: conversation!,
          ),
        ),
      );
      return;
    }
    setState(() {
      _section = _HomeSection.chats;
      _selectedConversationId = conversation!.id;
      _selectedContactId = contact.id;
    });
  }

  void _openJoinInvitation() =>
      showJoinInvitationModal(context, widget.gateway);

  void _openInvitationGenerator() =>
      showInvitationGeneratorModal(context, widget.gateway);

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

String _shortBuild(String value) =>
    value.length > 8 ? value.substring(0, 8) : value;

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
                          tooltip: 'Open chat',
                          onPressed: () => onOpenConversation(contact),
                          icon: Icon(context.torcaIcons.chats),
                        ),
                        IconButton(
                          tooltip: 'Contact information',
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
                onOpen: () => onOpenDetails(active),
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
  Widget build(BuildContext context) => ListView(
    padding: const EdgeInsets.all(24),
    children: <Widget>[
      Text('Invitations', style: Theme.of(context).textTheme.headlineSmall),
      const SizedBox(height: 8),
      const Text('Create and manage short-lived private contact invitations.'),
      const SizedBox(height: 20),
      FilledButton.icon(
        onPressed: onOpen,
        icon: Icon(context.torcaIcons.invitations),
        label: const Text('Generate Invitation'),
      ),
      const SizedBox(height: 24),
      if (pairings.isEmpty)
        _SectionEmptyState(
          icon: context.torcaIcons.invitations,
          title: 'No invitations',
          message:
              'Your active invitations and pairing requests will appear here.',
        )
      else ...<Widget>[
        Text(
          'Recent invitations',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 8),
        for (final pairing in pairings.reversed)
          Card(
            child: ListTile(
              leading: Icon(
                pairing.typedRole == PairingRole.creator
                    ? context.torcaIcons.invitations
                    : context.torcaIcons.link,
              ),
              title: Text(
                pairing.typedRole == PairingRole.creator
                    ? 'Created invitation'
                    : 'Joined invitation',
              ),
              subtitle: Text('Code ${pairing.code}'),
              trailing: Chip(label: Text(pairing.state)),
              onTap: () => onOpenInvitation(pairing),
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
        TorcaAvatar(label: contact.displayName, size: 56),
        const SizedBox(height: 14),
        Text(
          contact.displayName,
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 4),
        Text(_contactPresence(contact)),
        const SizedBox(height: 20),
        const Text('Connection', style: TextStyle(fontWeight: FontWeight.w600)),
        const SizedBox(height: 6),
        const SizedBox(height: 8),
        ConnectionIndicator(
          state: contact.connectionState,
          blocked: contact.typedStatus == ContactStatus.blocked,
        ),
        const SizedBox(height: 16),
        _ContextValue(label: 'Quality', value: contact.peerHealth.quality),
        _ContextValue(
          label: 'Round trip',
          value: contact.peerHealth.rttMs == null
              ? 'Not measured'
              : '${contact.peerHealth.rttMs} ms',
        ),
        _ContextValue(label: 'Presence', value: contact.presenceState),
        _ContextValue(
          label: 'Last seen',
          value: contact.lastSeenAtMs == null
              ? 'Never'
              : DateTime.fromMillisecondsSinceEpoch(
                  contact.lastSeenAtMs!,
                ).toLocal().toString(),
        ),
      ],
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
        const Text('Select a conversation'),
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
