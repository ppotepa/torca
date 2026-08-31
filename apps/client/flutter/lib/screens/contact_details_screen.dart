import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:torca_avatar/torca_avatar.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/connection_indicator.dart';
import '../widgets/contact_actions.dart';
import '../widgets/operation_tracker.dart';
import '../widgets/runtime_network_status.dart';
import 'connection_details_screen.dart';

class ContactDetailsScreen extends StatefulWidget {
  const ContactDetailsScreen({
    required this.gateway,
    required this.contact,
    super.key,
  });

  final EngineGateway gateway;
  final ContactDto contact;
  @override
  State<ContactDetailsScreen> createState() => _ContactDetailsScreenState();
}

class _ContactDetailsScreenState extends State<ContactDetailsScreen> {
  final OperationTracker _operations = OperationTracker();

  @override
  void initState() {
    super.initState();
    _operations.addListener(_changed);
  }

  @override
  void dispose() {
    _operations.removeListener(_changed);
    _operations.dispose();
    super.dispose();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: widget.gateway.snapshots,
    builder: (context, snapshot, _) {
      final contact = _currentContact(snapshot) ?? widget.contact;
      final conversationIds = snapshot.conversations
          .where((item) => item.contactId == contact.id)
          .map((item) => item.id)
          .toSet();
      final messageIds = snapshot.messages
          .where((item) => conversationIds.contains(item.conversationId))
          .map((item) => item.id)
          .toSet();
      final sharedAttachmentNames = snapshot.attachments
          .where((item) => messageIds.contains(item.messageId))
          .map((item) => item.name)
          .where((item) => item.trim().isNotEmpty)
          .toList(growable: false);
      return Scaffold(
        appBar: RuntimeAppBar(title: Text(contact.displayName)),
        body: ContactDetailsContent(
          contact: contact,
          sharedAttachmentNames: sharedAttachmentNames,
          onOpenConnectionDetails: () => Navigator.of(context).push<void>(
            MaterialPageRoute(
              builder: (_) => ConnectionDetailsScreen(
                gateway: widget.gateway,
                contactId: contact.id,
              ),
            ),
          ),
          onRename: () => _rename(contact),
          onToggleBlock: () => _toggleBlock(contact),
          onRemove: () => _remove(contact),
          onVerify: () => _verify(contact),
        ),
      );
    },
  );

  ContactDto? _currentContact(AppSnapshotDto snapshot) {
    for (final contact in snapshot.contacts) {
      if (contact.id == widget.contact.id) return contact;
    }
    return null;
  }

  Future<void> _rename(ContactDto contact) async {
    await ContactActions.rename(context, widget.gateway, contact);
  }

  Future<void> _toggleBlock(ContactDto contact) async {
    await ContactActions.toggleBlock(context, widget.gateway, contact);
  }

  Future<void> _remove(ContactDto contact) async {
    final removed = await ContactActions.remove(
      context,
      widget.gateway,
      contact,
    );
    if (mounted && removed) Navigator.of(context).pop();
  }

  Future<void> _verify(ContactDto contact) async {
    final command =
        contact.typedVerificationStatus == VerificationStatus.verified
        ? ResetContactVerificationCommandDto(contactIdHex: contact.id)
        : VerifyContactCommandDto(contactIdHex: contact.id);
    final result = await widget.gateway.execute(command);
    if (!mounted || result.ok) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(context.l10n.identityChanged)));
  }
}

/// Shared contact details content used by mobile routes and desktop panels.
/// Navigation and mutations stay outside so the same layout never owns a
/// second copy of contact lifecycle logic.
class ContactDetailsContent extends StatelessWidget {
  const ContactDetailsContent({
    required this.contact,
    this.sharedAttachmentNames = const <String>[],
    this.onOpenConnectionDetails,
    this.onVerify,
    this.onRename,
    this.onToggleBlock,
    this.onRemove,
    this.scrollable = true,
    super.key,
  });

  final ContactDto contact;
  final List<String> sharedAttachmentNames;
  final VoidCallback? onOpenConnectionDetails;
  final VoidCallback? onVerify;
  final VoidCallback? onRename;
  final VoidCallback? onToggleBlock;
  final VoidCallback? onRemove;
  final bool scrollable;

  @override
  Widget build(BuildContext context) {
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Card(
          child: ListTile(
            leading: TorcaDeviceAvatar(
              label: contact.displayName,
              identityId: contact.remoteIdentityId,
              fallbackIdentityId: contact.id,
              presentation: AvatarActivityPresentation.resolve(
                blocked: contact.typedStatus == ContactStatus.blocked,
                online: contact.typedAvailability == PeerAvailability.reachable,
              ),
            ),
            title: Text(contact.displayName),
            subtitle: Text(
              contact.typedStatus == ContactStatus.blocked
                  ? context.l10n.blocked
                  : context.l10n.directProviderContact(
                      contact.transportProvider,
                    ),
            ),
            trailing: ConnectionIndicator(
              state: contact.availabilityIndicatorState,
              blocked: contact.typedStatus == ContactStatus.blocked,
              provider: contact.transportProvider,
            ),
          ),
        ),
        const SizedBox(height: 12),
        if (sharedAttachmentNames.isNotEmpty) ...<Widget>[
          _DetailsCard(
            title: context.l10n.sharedMedia,
            children: <Widget>[
              Text(
                context.l10n.sharedMediaCount(sharedAttachmentNames.length),
              ),
              const SizedBox(height: 8),
              for (final name in sharedAttachmentNames.take(8))
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  dense: true,
                  leading: Icon(context.torcaIcons.attachment),
                  title: Text(
                    name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              if (sharedAttachmentNames.length > 8)
                Text('+ ${sharedAttachmentNames.length - 8}'),
            ],
          ),
          const SizedBox(height: 12),
        ],
        _DetailsCard(
          title: context.l10n.connection,
          children: <Widget>[
            _ValueRow(
              label: context.l10n.state,
              value: contact.connectionState,
            ),
            _ValueRow(
              label: context.l10n.quality,
              value: contact.peerHealth.quality,
            ),
            _ValueRow(
              label: context.l10n.transport,
              value: contact.transportProvider.toUpperCase(),
            ),
            _ValueRow(
              label: context.l10n.providerEndpoint,
              value: contact.endpointAvailable
                  ? context.l10n.providerEndpointAvailable
                  : context.l10n.providerEndpointUnavailable,
            ),
            if (onOpenConnectionDetails != null)
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: onOpenConnectionDetails,
                  icon: Icon(context.torcaIcons.diagnostics),
                  label: Text(context.l10n.connectionDetails),
                ),
              ),
          ],
        ),
        if (onVerify != null) ...<Widget>[
          const SizedBox(height: 12),
          _DetailsCard(
            title: context.l10n.verification,
            children: <Widget>[
              Text(
                contact.typedVerificationStatus == VerificationStatus.verified
                    ? context.l10n.verified
                    : context.l10n.unverified,
              ),
              if (contact.safetyNumber != null) ...<Widget>[
                const SizedBox(height: 8),
                SelectableText(contact.safetyNumber!),
              ],
              const SizedBox(height: 8),
              TextButton.icon(
                onPressed: onVerify,
                icon: Icon(
                  contact.typedVerificationStatus == VerificationStatus.verified
                      ? context.torcaIcons.remove
                      : context.torcaIcons.success,
                ),
                label: Text(
                  contact.typedVerificationStatus == VerificationStatus.verified
                      ? context.l10n.resetVerification
                      : context.l10n.verifyContact,
                ),
              ),
            ],
          ),
        ],
        const SizedBox(height: 12),
        _DetailsCard(
          title: context.l10n.contactActions,
          children: <Widget>[
            if (onRename != null)
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: Icon(context.torcaIcons.edit),
                title: Text(context.l10n.renameContact),
                onTap: onRename,
              ),
            if (onToggleBlock != null)
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: Icon(
                  contact.typedStatus == ContactStatus.blocked
                      ? context.torcaIcons.success
                      : context.torcaIcons.block,
                ),
                title: Text(
                  contact.typedStatus == ContactStatus.blocked
                      ? context.l10n.unblockContact
                      : context.l10n.blockContact,
                ),
                onTap: onToggleBlock,
              ),
            if (onRemove != null)
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: Icon(
                  context.torcaIcons.remove,
                  color: Theme.of(context).colorScheme.error,
                ),
                title: Text(
                  context.l10n.removeContact,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
                onTap: onRemove,
              ),
          ],
        ),
      ],
    );
    if (!scrollable) return content;
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 760),
        child: ListView(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
          children: <Widget>[content],
        ),
      ),
    );
  }
}

class IdentityDetailsScreen extends StatelessWidget {
  const IdentityDetailsScreen({
    required this.snapshot,
    this.buildInfo,
    super.key,
  });
  final AppSnapshotDto snapshot;
  final ClientBuildInfo? buildInfo;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: RuntimeAppBar(title: Text(context.l10n.yourIdentity)),
    body: Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 760),
        child: ListView(
          padding: const EdgeInsets.all(16),
          children: <Widget>[
            _DetailsCard(
              title: context.l10n.localIdentity,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    TorcaDeviceAvatar(
                      label:
                          snapshot.identity?.displayName ??
                          context.l10n.yourIdentity,
                      identityId: snapshot.identity?.id,
                      stableDevice: true,
                      size: 64,
                    ),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Text(
                        snapshot.identity?.displayName ??
                            context.l10n.unavailable,
                        style: Theme.of(context).textTheme.titleLarge,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 16),
                _ValueRow(
                  label: context.l10n.displayName,
                  value:
                      snapshot.identity?.displayName ??
                      context.l10n.unavailable,
                ),
                _ValueRow(
                  label: context.l10n.communicationProvider,
                  value: snapshot.communicationProvider.toUpperCase(),
                ),
                _ValueRow(
                  label: context.l10n.communicationState,
                  value: snapshot.communicationState,
                ),
                if (snapshot.endpointSummary != null)
                  _ValueRow(
                    label: context.l10n.endpoint,
                    value: snapshot.endpointSummary!,
                    selectable: true,
                  ),
                if (snapshot.identity?.fingerprint != null) ...<Widget>[
                  _ValueRow(
                    label: context.l10n.fingerprint,
                    value: snapshot.identity!.fingerprint!,
                    selectable: true,
                  ),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: TextButton.icon(
                      onPressed: () {
                        unawaited(
                          Clipboard.setData(
                            ClipboardData(
                              text: snapshot.identity!.fingerprint!,
                            ),
                          ),
                        );
                        if (context.mounted) {
                          ScaffoldMessenger.of(context).showSnackBar(
                            SnackBar(
                              content: Text(context.l10n.fingerprintCopied),
                            ),
                          );
                        }
                      },
                      icon: Icon(context.torcaIcons.copy),
                      label: Text(context.l10n.copyFingerprint),
                    ),
                  ),
                ],
              ],
            ),
            if (buildInfo != null) ...<Widget>[
              const SizedBox(height: 12),
              _DetailsCard(
                title: context.l10n.buildAndConnectionInfo,
                children: <Widget>[
                  _ValueRow(
                    label: context.l10n.productVersion,
                    value: buildInfo!.productVersion,
                  ),
                  _ValueRow(
                    label: context.l10n.build,
                    value: buildInfo!.buildId,
                    selectable: true,
                  ),
                  _ValueRow(
                    label: context.l10n.sourceCommit,
                    value: buildInfo!.sourceCommit,
                    selectable: true,
                  ),
                  _ValueRow(
                    label: context.l10n.contract,
                    value:
                        '${buildInfo!.contractSchema} / ${buildInfo!.wireVersion}',
                  ),
                  _ValueRow(
                    label: context.l10n.storageEpoch,
                    value: '${buildInfo!.storageEpoch}',
                  ),
                ],
              ),
            ],
          ],
        ),
      ),
    ),
  );
}

class _DetailsCard extends StatelessWidget {
  const _DetailsCard({required this.title, required this.children});
  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => Card(
    child: Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(title, style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 12),
          ...children,
        ],
      ),
    ),
  );
}

class _ValueRow extends StatelessWidget {
  const _ValueRow({
    required this.label,
    required this.value,
    this.selectable = false,
  });
  final String label;
  final String value;
  final bool selectable;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 10),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(label, style: Theme.of(context).textTheme.labelMedium),
        const SizedBox(height: 2),
        if (selectable) SelectableText(value) else Text(value),
      ],
    ),
  );
}


