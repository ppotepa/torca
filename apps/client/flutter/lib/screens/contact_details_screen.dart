import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../generated/torca_contract.dart';

class ContactDetailsScreen extends StatelessWidget {
  const ContactDetailsScreen({required this.contact, super.key});

  final ContactDto contact;

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Contact details')),
        body: ListView(
          padding: const EdgeInsets.all(24),
          children: <Widget>[
            const CircleAvatar(radius: 36, child: Icon(Icons.person_outline, size: 36)),
            const SizedBox(height: 20),
            _DetailTile(label: 'Contact ID', value: contact.id, copyable: true),
            _DetailTile(label: 'Connection', value: _connectionLabel(contact.connectionState)),
            _DetailTile(label: 'Relationship', value: contact.status),
            _DetailTile(label: 'Onion address', value: contact.onionAddress, copyable: true),
            const SizedBox(height: 16),
            const Text(
              'Direct messages are authenticated against the peer identity and are sent peer-to-peer through Tor.',
            ),
          ],
        ),
      );

  String _connectionLabel(String state) => switch (state) {
        'ready' => 'Direct P2P over Tor',
        'connecting' || 'handshaking' => 'Connecting',
        'reconnecting' => 'Reconnecting',
        _ => 'Offline',
      };
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
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(content: Text('$label copied')),
                      );
                    }
                  },
                )
              : null,
        ),
      );
