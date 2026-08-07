import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

class ContactDetailsScreen extends StatelessWidget {
  const ContactDetailsScreen({required this.gateway, required this.contact, super.key});

  final EngineGateway gateway;
  final ContactDto contact;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: gateway.snapshots,
    builder: (context, snapshot, _) {
      ContactDto? current;
      for (final item in snapshot.contacts) {
        if (item.id == contact.id) { current = item; break; }
      }
      if (current == null) {
        return Scaffold(
          appBar: AppBar(title: const Text('Contact details')),
          body: const Center(child: Text('This contact has been removed.')),
        );
      }
      final value = current;
      final safetyNumber = value.safetyNumber;
      final blocked = value.status == 'blocked';
      return Scaffold(
        appBar: AppBar(title: Text(value.displayName)),
        body: ListView(
          padding: const EdgeInsets.all(24),
          children: <Widget>[
            const CircleAvatar(radius: 36, child: Icon(Icons.person_outline, size: 36)),
            const SizedBox(height: 12),
            Center(child: Text(value.displayName, style: Theme.of(context).textTheme.headlineSmall)),
            const SizedBox(height: 12),
            Center(
              child: OutlinedButton.icon(
                onPressed: () => _rename(context, value),
                icon: const Icon(Icons.edit_outlined),
                label: const Text('Rename contact'),
              ),
            ),
            const SizedBox(height: 12),
            _DetailTile(label: 'Contact ID', value: value.id, copyable: true),
            _DetailTile(label: 'Connection', value: _connectionLabel(value.connectionState, blocked)),
            _DetailTile(label: 'Relationship', value: blocked ? 'Blocked' : value.status),
            _DetailTile(label: 'Onion address', value: value.onionAddress, copyable: true),
            if (safetyNumber != null && safetyNumber.isNotEmpty) ...<Widget>[
              _DetailTile(label: 'Safety number', value: safetyNumber, copyable: true),
              const Padding(
                padding: EdgeInsets.fromLTRB(12, 4, 12, 12),
                child: Text(
                  'Compare this value with your contact over another trusted channel. Both devices calculate the same value from their verified public identity keys.',
                ),
              ),
            ],
            const SizedBox(height: 16),
            FilledButton.tonalIcon(
              onPressed: () => blocked ? _unblock(context, value) : _block(context, value),
              icon: Icon(blocked ? Icons.check_circle_outline : Icons.block),
              label: Text(blocked ? 'Unblock contact' : 'Block contact'),
            ),
            const SizedBox(height: 12),
            OutlinedButton.icon(
              onPressed: () => _remove(context, value),
              icon: const Icon(Icons.person_remove_outlined),
              label: const Text('Remove contact and local history'),
            ),
            const SizedBox(height: 16),
            const Text(
              'Direct messages are authenticated against the peer identity and are sent peer-to-peer through Tor. Blocking closes active peer sessions and prevents reconnects.',
            ),
          ],
        ),
      );
    },
  );

  Future<void> _rename(BuildContext context, ContactDto value) async {
    final controller = TextEditingController(text: value.displayName);
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Rename contact'),
        content: TextField(
          controller: controller,
          autofocus: true,
          maxLength: 64,
          decoration: const InputDecoration(labelText: 'Local name'),
          onSubmitted: (text) => Navigator.of(context).pop(text),
        ),
        actions: <Widget>[
          TextButton(onPressed: () => Navigator.of(context).pop(), child: const Text('Cancel')),
          FilledButton(onPressed: () => Navigator.of(context).pop(controller.text), child: const Text('Save')),
        ],
      ),
    );
    controller.dispose();
    final normalized = name?.trim();
    if (normalized == null || normalized.isEmpty || !context.mounted) return;
    final result = await gateway.execute(RenameContactCommandDto(contactIdHex: value.id, displayName: normalized));
    if (context.mounted && !result.ok) _error(context, result.error ?? 'Could not rename contact');
  }

  Future<void> _block(BuildContext context, ContactDto value) async {
    final confirmed = await _confirm(
      context,
      'Block ${value.displayName}?',
      'The current peer connection will be closed and Torca will not reconnect until you unblock this contact.',
      'Block',
    );
    if (!confirmed || !context.mounted) return;
    final result = await gateway.execute(BlockContactCommandDto(contactIdHex: value.id));
    if (context.mounted && !result.ok) _error(context, result.error ?? 'Could not block contact');
  }

  Future<void> _unblock(BuildContext context, ContactDto value) async {
    final result = await gateway.execute(UnblockContactCommandDto(contactIdHex: value.id));
    if (context.mounted && !result.ok) _error(context, result.error ?? 'Could not unblock contact');
  }

  Future<void> _remove(BuildContext context, ContactDto value) async {
    final confirmed = await _confirm(
      context,
      'Remove ${value.displayName}?',
      'This removes the contact, conversation history, pending delivery/receipt work, attachment cache and the protected peer credential on this device. This cannot be undone.',
      'Remove',
    );
    if (!confirmed || !context.mounted) return;
    final result = await gateway.execute(RemoveContactCommandDto(contactIdHex: value.id));
    if (!context.mounted) return;
    if (result.ok) {
      Navigator.of(context).pop();
    } else {
      _error(context, result.error ?? 'Could not remove contact');
    }
  }

  Future<bool> _confirm(BuildContext context, String title, String message, String action) async =>
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

  void _error(BuildContext context, String message) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(message)));
  }

  String _connectionLabel(String state, bool blocked) {
    if (blocked) return 'Blocked';
    return switch (state) {
      'ready' => 'Direct P2P over Tor',
      'connecting' || 'handshaking' => 'Connecting',
      'reconnecting' => 'Reconnecting',
      _ => 'Offline',
    };
  }
}

class IdentityDetailsScreen extends StatelessWidget {
  const IdentityDetailsScreen({required this.snapshot, super.key});
  final AppSnapshotDto snapshot;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('Your Torca identity')),
    body: ListView(
      padding: const EdgeInsets.all(24),
      children: <Widget>[
        const CircleAvatar(radius: 36, child: Icon(Icons.shield_outlined, size: 36)),
        const SizedBox(height: 20),
        _DetailTile(label: 'Display name', value: snapshot.identity?.displayName ?? 'Not created'),
        _DetailTile(label: 'Tor', value: snapshot.torState),
        if (snapshot.onionAddress != null)
          _DetailTile(label: 'Onion address', value: snapshot.onionAddress!, copyable: true),
        const SizedBox(height: 16),
        const Text(
          'Private signing keys and pairwise secrets never leave protected platform storage. The onion address is safe to copy; secret material is never exposed here.',
        ),
      ],
    ),
  );
}

class _DetailTile extends StatelessWidget {
  const _DetailTile({required this.label, required this.value, this.copyable = false});
  final String label;
  final String value;
  final bool copyable;

  @override
  Widget build(BuildContext context) => Card(
    child: ListTile(
      title: Text(label),
      subtitle: SelectableText(value),
      trailing: copyable
          ? IconButton(
              tooltip: 'Copy',
              icon: const Icon(Icons.copy_outlined),
              onPressed: () async {
                await Clipboard.setData(ClipboardData(text: value));
                if (context.mounted) {
                  ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$label copied')));
                }
              },
            )
          : null,
    ),
  );
}
