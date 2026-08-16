import 'dart:async';

import 'package:flutter/material.dart';
import 'package:torca_avatar/torca_avatar.dart';
import 'package:torca_ui/torca_ui.dart';
import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
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
    ).showSnackBar(SnackBar(content: Text(context.strings.identityChanged)));
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
              presentation: AvatarActivityPresentation.resolve(
                blocked: contact.typedStatus == ContactStatus.blocked,
                online: contact.presenceState == 'online',
              ),
            ),
            title: Text(contact.displayName),
            subtitle: Text(
              contact.typedStatus == ContactStatus.blocked
                  ? context.strings.blocked
                  : context.strings.directTorContact,
            ),
            trailing: ConnectionIndicator(
              state: contact.connectionState,
              blocked: contact.typedStatus == ContactStatus.blocked,
            ),
          ),
        ),
        const SizedBox(height: 12),
        if (sharedAttachmentNames.isNotEmpty) ...<Widget>[
          _DetailsCard(
            title: context.strings.sharedMedia,
            children: <Widget>[
              Text(
                context.strings.sharedMediaCount(sharedAttachmentNames.length),
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
          title: context.strings.connection,
          children: <Widget>[
            _ValueRow(
              label: context.strings.state,
              value: contact.connectionState,
            ),
            _ValueRow(
              label: context.strings.quality,
              value: contact.peerHealth.quality,
            ),
            _ValueRow(
              label: context.strings.onionAddress,
              value: contact.onionAddress,
              selectable: true,
            ),
            if (onOpenConnectionDetails != null)
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: onOpenConnectionDetails,
                  icon: Icon(context.torcaIcons.diagnostics),
                  label: Text(context.strings.connectionDetails),
                ),
              ),
          ],
        ),
        if (onVerify != null) ...<Widget>[
          const SizedBox(height: 12),
          _DetailsCard(
            title: context.strings.verification,
            children: <Widget>[
              Text(
                contact.typedVerificationStatus == VerificationStatus.verified
                    ? context.strings.verified
                    : context.strings.unverified,
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
                      ? context.strings.resetVerification
                      : context.strings.verifyContact,
                ),
              ),
            ],
          ),
        ],
        const SizedBox(height: 12),
        _DetailsCard(
          title: context.strings.contactActions,
          children: <Widget>[
            if (onRename != null)
              ListTile(
                contentPadding: EdgeInsets.zero,
                leading: Icon(context.torcaIcons.edit),
                title: Text(context.strings.renameContact),
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
                      ? context.strings.unblockContact
                      : context.strings.blockContact,
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
                  context.strings.removeContact,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
                onTap: onRemove,
              ),
          ],
        ),
      ],
    );
    if (!scrollable) return content;
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
      children: <Widget>[content],
    );
  }
}

class IdentityDetailsScreen extends StatelessWidget {
  const IdentityDetailsScreen({required this.snapshot, super.key});
  final AppSnapshotDto snapshot;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: RuntimeAppBar(title: Text(context.strings.yourIdentity)),
    body: ListView(
      padding: const EdgeInsets.all(16),
      children: <Widget>[
        _DetailsCard(
          title: context.strings.localIdentity,
          children: <Widget>[
            _ValueRow(
              label: context.strings.displayName,
              value:
                  snapshot.identity?.displayName ?? context.strings.unavailable,
            ),
            _ValueRow(
              label: context.strings.torState,
              value: snapshot.torState,
            ),
            _ValueRow(
              label: context.strings.onionAddress,
              value: snapshot.onionAddress ?? context.strings.unavailable,
              selectable: true,
            ),
          ],
        ),
      ],
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
