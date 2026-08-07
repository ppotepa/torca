import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/connection_indicator.dart';
import '../widgets/operation_tracker.dart';
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
          final verificationBusy = _operations.isActive('verify:${contact.id}');
          return Scaffold(
            appBar: AppBar(title: Text(contact.displayName)),
            body: ListView(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
              children: <Widget>[
                Card(
                  child: ListTile(
                    leading: const CircleAvatar(child: Icon(Icons.person_outline)),
                    title: Text(contact.displayName),
                    subtitle: Text(contact.status == 'blocked' ? 'Blocked' : 'Direct Tor contact'),
                    trailing: ConnectionIndicator(
                      state: contact.connectionState,
                      blocked: contact.status == 'blocked',
                    ),
                  ),
                ),
                const SizedBox(height: 12),
                _DetailsCard(
                  title: 'Connection',
                  children: <Widget>[
                    _ValueRow(label: 'State', value: contact.connectionState),
                    _ValueRow(label: 'Quality', value: contact.peerHealth.quality),
                    _ValueRow(label: 'Onion address', value: contact.onionAddress, selectable: true),
                    Align(
                      alignment: Alignment.centerLeft,
                      child: TextButton.icon(
                        onPressed: () => Navigator.of(context).push<void>(
                          MaterialPageRoute(
                            builder: (_) => ConnectionDetailsScreen(
                              gateway: widget.gateway,
                              contactId: contact.id,
                            ),
                          ),
                        ),
                        icon: const Icon(Icons.network_check),
                        label: const Text('Connection details'),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                _DetailsCard(
                  title: 'Safety Number',
                  children: <Widget>[
                    SelectableText(
                      contact.safetyNumber?.isNotEmpty == true
                          ? contact.safetyNumber!
                          : 'Unavailable until the secure relationship is established.',
                    ),
                    const SizedBox(height: 12),
                    Row(
                      children: <Widget>[
                        Icon(
                          contact.verificationStatus == 'verified'
                              ? Icons.verified_user
                              : Icons.gpp_maybe_outlined,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            contact.verificationStatus == 'verified'
                                ? _verifiedLabel(contact.verifiedAtMs)
                                : 'Not verified on this device',
                          ),
                        ),
                        if (verificationBusy)
                          const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    Text(
                      contact.verificationStatus == 'verified'
                          ? 'Verification is local and is automatically invalid if the remote identity changes.'
                          : 'Compare the Safety Number with your contact through another trusted channel before marking it verified.',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    const SizedBox(height: 8),
                    Align(
                      alignment: Alignment.centerLeft,
                      child: contact.verificationStatus == 'verified'
                          ? OutlinedButton.icon(
                              onPressed: verificationBusy ? null : () => _setVerification(contact, false),
                              icon: const Icon(Icons.restart_alt),
                              label: const Text('Reset verification'),
                            )
                          : FilledButton.icon(
                              onPressed: verificationBusy ? null : () => _setVerification(contact, true),
                              icon: const Icon(Icons.verified_outlined),
                              label: const Text('Mark as verified'),
                            ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                _DetailsCard(
                  title: 'Contact actions',
                  children: <Widget>[
                    ListTile(
                      contentPadding: EdgeInsets.zero,
                      leading: const Icon(Icons.edit_outlined),
                      title: const Text('Rename contact'),
                      onTap: () => _rename(contact),
                    ),
                    ListTile(
                      contentPadding: EdgeInsets.zero,
                      leading: Icon(contact.status == 'blocked' ? Icons.lock_open : Icons.block),
                      title: Text(contact.status == 'blocked' ? 'Unblock contact' : 'Block contact'),
                      onTap: () => _toggleBlock(contact),
                    ),
                    ListTile(
                      contentPadding: EdgeInsets.zero,
                      leading: Icon(Icons.delete_outline, color: Theme.of(context).colorScheme.error),
                      title: Text('Remove contact', style: TextStyle(color: Theme.of(context).colorScheme.error)),
                      onTap: () => _remove(contact),
                    ),
                  ],
                ),
              ],
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

  String _verifiedLabel(int? milliseconds) {
    if (milliseconds == null || milliseconds <= 0) return 'Verified on this device';
    final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    return 'Verified ${date.year}-${date.month.toString().padLeft(2, '0')}-${date.day.toString().padLeft(2, '0')}';
  }

  Future<void> _setVerification(ContactDto contact, bool verified) async {
    await _operations.run('verify:${contact.id}', () async {
      final result = await widget.gateway.execute(
        verified
            ? VerifyContactCommandDto(contactIdHex: contact.id)
            : ResetContactVerificationCommandDto(contactIdHex: contact.id),
      );
      if (mounted && !result.ok) _showError(result, 'Could not update verification');
    });
  }

  Future<void> _rename(ContactDto contact) async {
    final controller = TextEditingController(text: contact.displayName);
    final value = await showDialog<String>(
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
          TextButton(onPressed: () => Navigator.of(context).pop(), child: const Text('Cancel')),
          FilledButton(onPressed: () => Navigator.of(context).pop(controller.text), child: const Text('Save')),
        ],
      ),
    );
    controller.dispose();
    final name = value?.trim();
    if (name == null || name.isEmpty || !mounted) return;
    final result = await widget.gateway.execute(
      RenameContactCommandDto(contactIdHex: contact.id, displayName: name),
    );
    if (mounted && !result.ok) _showError(result, 'Could not rename contact');
  }

  Future<void> _toggleBlock(ContactDto contact) async {
    final blocking = contact.status != 'blocked';
    if (blocking && !await _confirm('Block ${contact.displayName}?', 'Torca will close the peer connection and will not reconnect until you unblock this contact.', 'Block')) return;
    final result = await widget.gateway.execute(
      blocking ? BlockContactCommandDto(contactIdHex: contact.id) : UnblockContactCommandDto(contactIdHex: contact.id),
    );
    if (mounted && !result.ok) _showError(result, blocking ? 'Could not block contact' : 'Could not unblock contact');
  }

  Future<void> _remove(ContactDto contact) async {
    if (!await _confirm('Remove ${contact.displayName}?', 'This removes the local relationship, conversation history, pending work and protected peer credential.', 'Remove')) return;
    final result = await widget.gateway.execute(RemoveContactCommandDto(contactIdHex: contact.id));
    if (!mounted) return;
    if (!result.ok) { _showError(result, 'Could not remove contact'); return; }
    Navigator.of(context).pop();
  }

  Future<bool> _confirm(String title, String message, String action) async =>
      await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(title),
          content: Text(message),
          actions: <Widget>[
            TextButton(onPressed: () => Navigator.of(context).pop(false), child: const Text('Cancel')),
            FilledButton(onPressed: () => Navigator.of(context).pop(true), child: Text(action)),
          ],
        ),
      ) ?? false;

  void _showError(BridgeResultDto result, String fallback) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(BridgeErrorPresenter.message(result, fallback: fallback))),
    );
  }
}

class IdentityDetailsScreen extends StatelessWidget {
  const IdentityDetailsScreen({required this.snapshot, super.key});
  final AppSnapshotDto snapshot;

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Your identity')),
        body: ListView(
          padding: const EdgeInsets.all(16),
          children: <Widget>[
            _DetailsCard(
              title: 'Local identity',
              children: <Widget>[
                _ValueRow(label: 'Display name', value: snapshot.identity?.displayName ?? 'Unavailable'),
                _ValueRow(label: 'Tor state', value: snapshot.torState),
                _ValueRow(label: 'Onion address', value: snapshot.onionAddress ?? 'Unavailable', selectable: true),
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
  const _ValueRow({required this.label, required this.value, this.selectable = false});
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
