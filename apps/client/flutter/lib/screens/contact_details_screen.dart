import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/connection_indicator.dart';
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
      final verificationBusy = _operations.isActive('verify:${contact.id}');
      final changed = contact.verificationStatus == 'changed';
      final verified = contact.verificationStatus == 'verified';
      final verificationUri = _verificationUri(contact);
      return Scaffold(
        appBar: RuntimeAppBar(title: Text(contact.displayName)),
        body: ListView(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
          children: <Widget>[
            if (changed) ...<Widget>[
              Card(
                color: Theme.of(context).colorScheme.errorContainer,
                child: const ListTile(
                  leading: Icon(Icons.gpp_bad_outlined),
                  title: Text('Contact identity changed'),
                  subtitle: Text(
                    'Sending is blocked until you compare and verify the new Safety Number.',
                  ),
                ),
              ),
              const SizedBox(height: 12),
            ],
            Card(
              child: ListTile(
                leading: const CircleAvatar(child: Icon(Icons.person_outline)),
                title: Text(contact.displayName),
                subtitle: Text(
                  contact.status == 'blocked'
                      ? 'Blocked'
                      : 'Direct Tor contact',
                ),
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
                _ValueRow(
                  label: 'Onion address',
                  value: contact.onionAddress,
                  selectable: true,
                ),
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
                if (verificationUri != null) ...<Widget>[
                  const SizedBox(height: 12),
                  Center(
                    child: QrImageView(
                      data: verificationUri,
                      size: 180,
                      semanticsLabel: 'Torca Safety Number verification QR',
                    ),
                  ),
                ],
                const SizedBox(height: 12),
                Row(
                  children: <Widget>[
                    Icon(
                      verified
                          ? Icons.verified_user
                          : changed
                          ? Icons.gpp_bad_outlined
                          : Icons.gpp_maybe_outlined,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        verified
                            ? _verifiedLabel(contact.verifiedAtMs)
                            : changed
                            ? 'Previously verified identity changed'
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
                  verified
                      ? 'Verification is local and is automatically invalid if the remote identity changes.'
                      : changed
                      ? 'Do not send sensitive information until you verify the new Safety Number through another trusted channel.'
                      : 'Compare this Safety Number through another trusted channel. Scanning the matching Torca QR verifies it without sending the number anywhere.',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 10),
                if (verified)
                  Align(
                    alignment: Alignment.centerLeft,
                    child: OutlinedButton.icon(
                      onPressed: verificationBusy
                          ? null
                          : () => _setVerification(contact, false),
                      icon: const Icon(Icons.restart_alt),
                      label: const Text('Reset verification'),
                    ),
                  )
                else
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: <Widget>[
                      FilledButton.icon(
                        onPressed: verificationBusy || verificationUri == null
                            ? null
                            : () => _scanAndVerify(contact),
                        icon: const Icon(Icons.qr_code_scanner),
                        label: const Text('Scan to verify'),
                      ),
                      OutlinedButton.icon(
                        onPressed: verificationBusy || verificationUri == null
                            ? null
                            : () => _confirmManualVerification(contact),
                        icon: const Icon(Icons.verified_outlined),
                        label: const Text('Compared manually'),
                      ),
                    ],
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
                  leading: Icon(
                    contact.status == 'blocked' ? Icons.lock_open : Icons.block,
                  ),
                  title: Text(
                    contact.status == 'blocked'
                        ? 'Unblock contact'
                        : 'Block contact',
                  ),
                  onTap: () => _toggleBlock(contact),
                ),
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: Icon(
                    Icons.delete_outline,
                    color: Theme.of(context).colorScheme.error,
                  ),
                  title: Text(
                    'Remove contact',
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
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
    if (milliseconds == null || milliseconds <= 0)
      return 'Verified on this device';
    final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    return 'Verified ${date.year}-${date.month.toString().padLeft(2, '0')}-${date.day.toString().padLeft(2, '0')}';
  }

  String? _verificationUri(ContactDto contact) {
    final safety = _normalizedSafety(contact.safetyNumber);
    if (safety == null) return null;
    return Uri(
      scheme: 'torca',
      host: 'verify',
      queryParameters: <String, String>{'v': '1', 'safety': safety},
    ).toString();
  }

  String? _normalizedSafety(String? value) {
    final normalized = value?.replaceAll(RegExp(r'\s+'), '').toUpperCase();
    if (normalized == null ||
        normalized.isEmpty ||
        !RegExp(r'^[0-9A-F]+$').hasMatch(normalized)) {
      return null;
    }
    return normalized;
  }

  Future<void> _scanAndVerify(ContactDto contact) async {
    final expected = _normalizedSafety(contact.safetyNumber);
    if (expected == null) return;
    final scanned = await showDialog<String>(
      context: context,
      builder: (context) => Dialog(
        child: SizedBox(
          width: 420,
          height: 520,
          child: Stack(
            children: <Widget>[
              MobileScanner(
                onDetect: (capture) {
                  for (final barcode in capture.barcodes) {
                    final raw = barcode.rawValue;
                    if (raw != null && _safetyFromUri(raw) != null) {
                      Navigator.of(context).pop(raw);
                      return;
                    }
                  }
                },
              ),
              Positioned(
                right: 8,
                top: 8,
                child: IconButton.filledTonal(
                  tooltip: 'Close scanner',
                  onPressed: () => Navigator.of(context).pop(),
                  icon: const Icon(Icons.close),
                ),
              ),
            ],
          ),
        ),
      ),
    );
    if (!mounted || scanned == null) return;
    if (_safetyFromUri(scanned) != expected) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Safety Number does not match this contact.'),
        ),
      );
      return;
    }
    await _setVerification(contact, true);
  }

  String? _safetyFromUri(String value) {
    final uri = Uri.tryParse(value.trim());
    if (uri == null ||
        uri.scheme != 'torca' ||
        uri.host != 'verify' ||
        uri.queryParameters['v'] != '1') {
      return null;
    }
    return _normalizedSafety(uri.queryParameters['safety']);
  }

  Future<void> _confirmManualVerification(ContactDto contact) async {
    final confirmed = await _confirm(
      'Verify Safety Number?',
      'Only continue if you compared the full Safety Number with this contact through another trusted channel.',
      'Mark verified',
    );
    if (confirmed) await _setVerification(contact, true);
  }

  Future<void> _setVerification(ContactDto contact, bool verified) async {
    await _operations.run('verify:${contact.id}', () async {
      final result = await widget.gateway.execute(
        verified
            ? VerifyContactCommandDto(contactIdHex: contact.id)
            : ResetContactVerificationCommandDto(contactIdHex: contact.id),
      );
      if (mounted && !result.ok)
        _showError(result, 'Could not update verification');
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
    final name = value?.trim();
    if (name == null || name.isEmpty || !mounted) return;
    final result = await widget.gateway.execute(
      RenameContactCommandDto(contactIdHex: contact.id, displayName: name),
    );
    if (mounted && !result.ok) _showError(result, 'Could not rename contact');
  }

  Future<void> _toggleBlock(ContactDto contact) async {
    final blocking = contact.status != 'blocked';
    if (blocking &&
        !await _confirm(
          'Block ${contact.displayName}?',
          'Torca will close the peer connection and will not reconnect until you unblock this contact.',
          'Block',
        ))
      return;
    final result = await widget.gateway.execute(
      blocking
          ? BlockContactCommandDto(contactIdHex: contact.id)
          : UnblockContactCommandDto(contactIdHex: contact.id),
    );
    if (mounted && !result.ok) {
      _showError(
        result,
        blocking ? 'Could not block contact' : 'Could not unblock contact',
      );
    }
  }

  Future<void> _remove(ContactDto contact) async {
    if (!await _confirm(
      'Remove ${contact.displayName}?',
      'This removes the local relationship, conversation history, pending work and protected peer credential.',
      'Remove',
    ))
      return;
    final result = await widget.gateway.execute(
      RemoveContactCommandDto(contactIdHex: contact.id),
    );
    if (!mounted) return;
    if (!result.ok) {
      _showError(result, 'Could not remove contact');
      return;
    }
    Navigator.of(context).pop();
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

  void _showError(BridgeResultDto result, String fallback) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(BridgeErrorPresenter.message(result, fallback: fallback)),
      ),
    );
  }
}

class IdentityDetailsScreen extends StatelessWidget {
  const IdentityDetailsScreen({required this.snapshot, super.key});
  final AppSnapshotDto snapshot;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: const RuntimeAppBar(title: Text('Your identity')),
    body: ListView(
      padding: const EdgeInsets.all(16),
      children: <Widget>[
        _DetailsCard(
          title: 'Local identity',
          children: <Widget>[
            _ValueRow(
              label: 'Display name',
              value: snapshot.identity?.displayName ?? 'Unavailable',
            ),
            _ValueRow(label: 'Tor state', value: snapshot.torState),
            _ValueRow(
              label: 'Onion address',
              value: snapshot.onionAddress ?? 'Unavailable',
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
