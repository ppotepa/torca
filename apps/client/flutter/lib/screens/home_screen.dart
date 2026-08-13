import 'dart:async';

import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
import '../settings/local_preferences.dart';
import '../widgets/adaptive_app_shell.dart';
import '../widgets/app_overflow_menu.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/connection_indicator.dart';
import '../widgets/contact_actions.dart';
import '../widgets/conversation_actions.dart';
import '../widgets/conversation_summary_tile.dart';
import '../widgets/runtime_network_status.dart';
import 'connection_details_screen.dart';
import 'contact_details_screen.dart';
import 'conversation_screen.dart';
import 'diagnostics_screen.dart';
import 'pairing_screen.dart';
import 'settings_screen.dart';

part 'home_bootstrap.dart';
part 'home_sections.dart';

const double _wideLayoutBreakpoint = 960;
const String _buildId = String.fromEnvironment(
  'TORCA_BUILD_ID',
  defaultValue: 'dev',
);

enum _HomeSection { chats, contacts, invitations }

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
      title: Text(context.strings.buildAndConnectionInfo),
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
          child: Text(context.strings.close),
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
                tooltip: context.strings.pairContact,
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
          selectedConversationId: _selectedConversationId,
          onContactInfo: _openContactDetails,
          onAction: _handleConversationAction,
          onSelected: (conversation) {
            // Keep selection in the shell even when the narrow layout uses a
            // route.  This lets a later resize restore the same conversation
            // instead of falling back to the contacts/list view.
            setState(() {
              _selectedConversationId = conversation.id;
              _selectedContactId = conversation.contactId;
            });
            Navigator.of(context).push<void>(
              MaterialPageRoute(
                builder: (_) => ConversationScreen(
                  gateway: widget.gateway,
                  conversation: conversation,
                ),
              ),
            );
          },
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
                onOpenConversation: () => _openConversationForContact(contact),
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
      case ContactAction.connectionDetails:
        Navigator.of(context).push<void>(
          MaterialPageRoute(
            builder: (_) => ConnectionDetailsScreen(
              gateway: widget.gateway,
              contactId: contact.id,
            ),
          ),
        );
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
              child: Text(context.strings.cancel),
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
      // The desktop context panel is derived from the selected conversation.
      // Keep that selection in sync with the explicit info action; otherwise
      // pressing "i" beside one conversation can leave details for a
      // previously selected contact visible in the fourth pane.
      ConversationDto? conversation;
      for (final candidate in widget.gateway.snapshots.value.conversations) {
        if (candidate.contactId == contact.id) {
          conversation = candidate;
          break;
        }
      }
      setState(() {
        _selectedContactId = contact.id;
        if (conversation != null) {
          _section = _HomeSection.chats;
          _selectedConversationId = conversation.id;
        } else {
          _section = _HomeSection.contacts;
        }
      });
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
