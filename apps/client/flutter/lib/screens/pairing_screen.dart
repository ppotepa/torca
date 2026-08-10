import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/app_modal.dart';
import '../widgets/async_action_button.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/operation_tracker.dart';
import '../widgets/pairing_progress.dart';
import '../widgets/runtime_network_status.dart';
import 'conversation_screen.dart';

enum _PairingMode { create, join }

class PairingScreen extends StatefulWidget {
  const PairingScreen({required this.gateway, super.key});
  final EngineGateway gateway;

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final TextEditingController _code = TextEditingController();
  final OperationTracker _operations = OperationTracker();
  _PairingMode _mode = _PairingMode.create;
  String? _error;

  @override
  void initState() {
    super.initState();
    _operations.addListener(_operationChanged);
  }

  @override
  void dispose() {
    _operations.removeListener(_operationChanged);
    _operations.dispose();
    _code.dispose();
    super.dispose();
  }

  void _operationChanged() {
    if (mounted) setState(() {});
  }

  bool get _primaryBusy =>
      _operations.isActive('pairing:create') ||
      _operations.isActive('pairing:join') ||
      _operations.isActive('pairing:scan');

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: const RuntimeAppBar(title: Text('Pair contact')),
    body: ValueListenableBuilder<AppSnapshotDto>(
      valueListenable: widget.gateway.snapshots,
      builder: (context, snapshot, _) {
        final relayReady = snapshot.transport.relay.isUsable;
        final relayState = snapshot.transport.relay.state;
        return ListView(
          padding: const EdgeInsets.all(24),
          children: <Widget>[
            Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 560),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    if (!relayReady) ...<Widget>[
                      Card(
                        color: Theme.of(context).colorScheme.errorContainer,
                        child: ListTile(
                          leading: const Icon(Icons.hub_outlined),
                          title: const Text('Secure relay is not connected'),
                          subtitle: Text(
                            relayState == 'checking'
                                ? 'Torca is verifying the relay through Tor. Creating or joining invitations will unlock automatically when it succeeds.'
                                : 'Creating and joining invitations are paused until the relay is reachable through Tor.',
                          ),
                        ),
                      ),
                      const SizedBox(height: 16),
                    ],
                    const Card(
                      child: ListTile(
                        leading: Icon(Icons.restart_alt),
                        title: Text('Pairing invitations are temporary'),
                        subtitle: Text(
                          'Each invitation expires after five minutes. You can cancel it at any time; completed, rejected and expired sessions disappear from this list.',
                        ),
                      ),
                    ),
                    const SizedBox(height: 16),
                    SegmentedButton<_PairingMode>(
                      segments: const <ButtonSegment<_PairingMode>>[
                        ButtonSegment(
                          value: _PairingMode.create,
                          icon: Icon(Icons.qr_code_2),
                          label: Text('Create invitation'),
                        ),
                        ButtonSegment(
                          value: _PairingMode.join,
                          icon: Icon(Icons.qr_code_scanner),
                          label: Text('Join invitation'),
                        ),
                      ],
                      selected: <_PairingMode>{_mode},
                      onSelectionChanged: _primaryBusy || !relayReady
                          ? null
                          : (value) => setState(() {
                              _mode = value.single;
                              _error = null;
                            }),
                    ),
                    const SizedBox(height: 20),
                    if (_mode == _PairingMode.create) ...<Widget>[
                      const Text(
                        'Create a short-lived invitation. The secure Rust runtime owns its ID, code and expiry.',
                      ),
                      const SizedBox(height: 12),
                      AsyncActionButton(
                        onPressed: _primaryBusy || !relayReady ? null : _create,
                        busy: _operations.isActive('pairing:create'),
                        icon: Icons.add_link,
                        label: 'Create invitation',
                      ),
                    ] else ...<Widget>[
                      TextField(
                        controller: _code,
                        enabled: !_primaryBusy && relayReady,
                        textCapitalization: TextCapitalization.characters,
                        autocorrect: false,
                        enableSuggestions: false,
                        decoration: InputDecoration(
                          labelText: 'Pairing code or Torca QR URI',
                          helperText:
                              'Enter the code or scan the QR shown by your contact.',
                          errorText: _error,
                          suffixIcon: IconButton(
                            tooltip: 'Scan QR',
                            onPressed: _primaryBusy || !relayReady
                                ? null
                                : _scan,
                            icon: _operations.isActive('pairing:scan')
                                ? const SizedBox(
                                    width: 18,
                                    height: 18,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.qr_code_scanner),
                          ),
                        ),
                        onSubmitted: _primaryBusy || !relayReady
                            ? null
                            : (_) => _join(),
                      ),
                      const SizedBox(height: 12),
                      FilledButton.icon(
                        onPressed: _primaryBusy || !relayReady ? null : _join,
                        icon: _operations.isActive('pairing:join')
                            ? const SizedBox(
                                width: 18,
                                height: 18,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.link),
                        label: Text(
                          _operations.isActive('pairing:join')
                              ? 'Joining…'
                              : 'Join invitation',
                        ),
                      ),
                    ],
                    if (_error != null &&
                        _mode == _PairingMode.create) ...<Widget>[
                      const SizedBox(height: 12),
                      Text(
                        _error!,
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ),
                    ],
                    if (snapshot.pairings.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 32),
                      Text(
                        'Pairing sessions',
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 12),
                      ...snapshot.pairings.reversed.map(
                        (pairing) => _PairingSessionCard(
                          pairing: pairing,
                          busy: _operations.anyWithPrefix(
                            'pairing:${pairing.id}:',
                          ),
                          onOpen: () => _showSession(pairing),
                          onApprove: () => _session(
                            pairing.id,
                            'approve',
                            ApprovePairingCommandDto(sessionIdHex: pairing.id),
                          ),
                          onReject: () => _session(
                            pairing.id,
                            'reject',
                            RejectPairingCommandDto(sessionIdHex: pairing.id),
                          ),
                          onCancel: () => _session(
                            pairing.id,
                            'cancel',
                            CancelPairingCommandDto(sessionIdHex: pairing.id),
                          ),
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ],
        );
      },
    ),
  );

  Future<void> _create() async {
    final result = await _run(
      'pairing:create',
      const CreatePairingCommandDto(),
    );
    if (result?.ok != true || !mounted) return;
    // The response publishes its post-transaction snapshot before succeeding.
    // Open the newly-created invitation immediately instead of making the user
    // locate its session card in a growing list.
    PairingDto? invitation;
    for (final pairing in widget.gateway.snapshots.value.pairings.reversed) {
      if (pairing.role == 'creator') {
        invitation = pairing;
        break;
      }
    }
    if (invitation != null) await _showSession(invitation);
  }

  Future<void> _showSession(PairingDto pairing) async {
    final knownConversationIds = widget.gateway.snapshots.value.conversations
        .map((conversation) => conversation.id)
        .toSet();
    final destination = await showDialog<String>(
      context: context,
      builder: (_) => ValueListenableBuilder<AppSnapshotDto>(
        valueListenable: widget.gateway.snapshots,
        builder: (context, snapshot, _) {
          PairingDto? current;
          for (final session in snapshot.pairings) {
            if (session.id == pairing.id) {
              current = session;
              break;
            }
          }
          if (current == null) {
            ConversationDto? completedConversation;
            for (final conversation in snapshot.conversations) {
              if (!knownConversationIds.contains(conversation.id)) {
                completedConversation = conversation;
                break;
              }
            }
            final completed = completedConversation != null;
            return AppModal(
              title: completed ? 'Contact added' : 'Pairing finished',
              height: 320,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Icon(
                    completed
                        ? Icons.mark_chat_unread_outlined
                        : Icons.check_circle_outline,
                    size: 48,
                  ),
                  const SizedBox(height: 14),
                  Text(
                    completed
                        ? 'Your contact was added securely. Open the new private conversation now.'
                        : 'This temporary invitation is no longer active.',
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 20),
                  FilledButton.icon(
                    onPressed: () => Navigator.of(context).pop(
                      completed
                          ? 'conversation:${completedConversation!.id}'
                          : 'close',
                    ),
                    icon: const Icon(Icons.forum_outlined),
                    label: Text(completed ? 'Open conversation' : 'Close'),
                  ),
                ],
              ),
            );
          }
          return AppModal(
            title: current.role == 'creator'
                ? 'Your invitation'
                : 'Pairing session',
            child: _PairingSessionCard(
              pairing: current,
              busy: _operations.anyWithPrefix('pairing:${current.id}:'),
              expanded: true,
              onApprove: () => _session(
                current!.id,
                'approve',
                ApprovePairingCommandDto(sessionIdHex: current.id),
              ),
              onReject: () => _session(
                current!.id,
                'reject',
                RejectPairingCommandDto(sessionIdHex: current.id),
              ),
              onCancel: () => _session(
                current!.id,
                'cancel',
                CancelPairingCommandDto(sessionIdHex: current.id),
              ),
            ),
          );
        },
      ),
    );
    if (!mounted || destination == null) return;
    const prefix = 'conversation:';
    if (!destination.startsWith(prefix)) return;
    final conversationId = destination.substring(prefix.length);
    ConversationDto? conversation;
    for (final item in widget.gateway.snapshots.value.conversations) {
      if (item.id == conversationId) {
        conversation = item;
        break;
      }
    }
    if (conversation == null) return;
    await Navigator.of(context).pushReplacement<void, void>(
      MaterialPageRoute(
        builder: (_) => ConversationScreen(
          gateway: widget.gateway,
          conversation: conversation!,
        ),
      ),
    );
  }

  Future<void> _join() async {
    final raw = _code.text.trim();
    final parser = widget.gateway is PairingUriParser
        ? widget.gateway as PairingUriParser
        : null;
    final code = raw.toLowerCase().startsWith('torca://')
        ? await parser?.parsePairingUri(raw)
        : _extractCode(raw);
    if (code == null) {
      setState(
        () => _error = 'Use a five-character code or a valid Torca QR URI',
      );
      return;
    }
    final result = await _run(
      'pairing:join',
      JoinPairingCommandDto(code: code),
    );
    if (result?.ok == true && mounted) _code.clear();
  }

  Future<void> _scan() async {
    await _operations.run('pairing:scan', () async {
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
                      final value = barcode.rawValue;
                      if (value != null && _extractCode(value) != null) {
                        Navigator.of(context).pop(value);
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
      if (scanned == null || !mounted) return;
      _code.text = _extractCode(scanned) ?? '';
    });
    if (_code.text.isNotEmpty && mounted) await _join();
  }

  Future<BridgeResultDto?> _run(String key, BridgeCommandDto command) async {
    BridgeResultDto? result;
    await _operations.run(key, () async {
      if (mounted) setState(() => _error = null);
      result = await widget.gateway.execute(command);
      if (mounted && result?.ok == false) {
        setState(() {
          _error = BridgeErrorPresenter.message(
            result!,
            fallback: 'Pairing operation failed',
          );
        });
      }
    });
    return result;
  }

  Future<void> _session(
    String sessionId,
    String action,
    BridgeCommandDto command,
  ) async {
    await _run('pairing:$sessionId:$action', command);
  }

  String? _extractCode(String input) {
    final value = input.trim();
    final direct = value
        .toUpperCase()
        .replaceAll(RegExp(r'[\s-]'), '')
        .replaceAll('O', '0')
        .replaceAll(RegExp('[IL]'), '1');
    if (RegExp(r'^[0-9A-HJKMNPQRSTVWXYZ]{5}$').hasMatch(direct)) {
      return direct;
    }
    final uri = Uri.tryParse(value);
    if (uri == null ||
        uri.scheme != 'torca' ||
        uri.host != 'pair' ||
        uri.queryParameters['v'] != '1') {
      return null;
    }
    final code = uri.queryParameters['code'];
    return code == null ? null : _extractCode(code);
  }
}

class _PairingSessionCard extends StatelessWidget {
  const _PairingSessionCard({
    required this.pairing,
    required this.busy,
    required this.onApprove,
    required this.onReject,
    required this.onCancel,
    this.onOpen,
    this.expanded = false,
  });
  final PairingDto pairing;
  final bool busy;
  final VoidCallback onApprove;
  final VoidCallback onReject;
  final VoidCallback onCancel;
  final VoidCallback? onOpen;
  final bool expanded;

  bool get _terminal => const {
    'rejected',
    'cancelled',
    'expired',
    'completed',
  }.contains(pairing.state);
  String get _uri =>
      'torca://pair?v=1&code=${Uri.encodeQueryComponent(pairing.code)}';
  String get _displayCode => pairing.code.length == 6
      ? '${pairing.code.substring(0, 3)}-${pairing.code.substring(3)}'
      : pairing.code;

  @override
  Widget build(BuildContext context) {
    if (!expanded) {
      return Card(
        margin: const EdgeInsets.only(bottom: 10),
        clipBehavior: Clip.antiAlias,
        child: ListTile(
          onTap: onOpen,
          leading: Icon(
            pairing.role == 'creator' ? Icons.qr_code_2 : Icons.link,
          ),
          title: Text(
            pairing.role == 'creator'
                ? 'Invitation $_displayCode'
                : 'Joined $_displayCode',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          subtitle: Text(
            '${_stateHelp(pairing.state)}\n${_expiryLabel(pairing.expiresAtMs)}',
          ),
          isThreeLine: true,
          trailing: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              if (busy)
                const Padding(
                  padding: EdgeInsets.only(right: 8),
                  child: SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                ),
              Chip(label: Text(_stateLabel(pairing.state))),
              const Icon(Icons.chevron_right),
            ],
          ),
        ),
      );
    }
    final canReview =
        pairing.state == 'awaitingapproval' ||
        pairing.state == 'awaiting_approval';
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: SelectableText(
                    _displayCode,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                if (busy)
                  const Padding(
                    padding: EdgeInsets.only(right: 8),
                    child: SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ),
                Chip(label: Text(_stateLabel(pairing.state))),
              ],
            ),
            Text(
              '${pairing.role == 'creator' ? 'Created invitation' : 'Joined invitation'} · ${_expiryLabel(pairing.expiresAtMs)}',
            ),
            const SizedBox(height: 10),
            PairingProgress(state: pairing.state),
            const SizedBox(height: 10),
            Text(_stateHelp(pairing.state)),
            if (pairing.role == 'creator' && !_terminal) ...<Widget>[
              const SizedBox(height: 12),
              Center(
                child: QrImageView(
                  data: _uri,
                  size: 190,
                  semanticsLabel: 'Torca pairing invitation',
                ),
              ),
              const SizedBox(height: 8),
              SelectableText(_uri, textAlign: TextAlign.center),
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                alignment: WrapAlignment.center,
                children: <Widget>[
                  OutlinedButton.icon(
                    icon: const Icon(Icons.copy_outlined),
                    label: const Text('Copy code'),
                    onPressed: busy
                        ? null
                        : () => _copy(
                            context,
                            pairing.code,
                            'Pairing code copied',
                          ),
                  ),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.link),
                    label: const Text('Copy invitation'),
                    onPressed: busy
                        ? null
                        : () => _copy(context, _uri, 'Invitation copied'),
                  ),
                ],
              ),
            ],
            if (canReview && pairing.role == 'creator') ...<Widget>[
              const SizedBox(height: 12),
              const Text(
                'A device joined using your invitation. Review and approve to create the contact on both devices.',
              ),
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                children: <Widget>[
                  FilledButton(
                    onPressed: busy ? null : onApprove,
                    child: const Text('Approve'),
                  ),
                  OutlinedButton(
                    onPressed: busy ? null : onReject,
                    child: const Text('Reject'),
                  ),
                ],
              ),
            ] else if (canReview) ...<Widget>[
              const SizedBox(height: 12),
              const Text(
                'Your join request was sent. The invitation owner must approve it before either contact is created.',
              ),
              const SizedBox(height: 8),
              TextButton(
                onPressed: busy ? null : onCancel,
                child: const Text('Cancel request'),
              ),
            ] else if (!_terminal) ...<Widget>[
              const SizedBox(height: 8),
              TextButton(
                onPressed: busy ? null : onCancel,
                child: const Text('Cancel'),
              ),
            ],
          ],
        ),
      ),
    );
  }

  Future<void> _copy(BuildContext context, String value, String message) async {
    await Clipboard.setData(ClipboardData(text: value));
    if (context.mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(message)));
    }
  }

  String _stateLabel(String state) => switch (state) {
    'open' => 'Waiting',
    'peerjoined' || 'peer_joined' => 'Peer joined',
    'awaitingapproval' || 'awaiting_approval' => 'Review peer',
    'approved' => 'Approved',
    'completed' => 'Connected',
    'rejected' => 'Rejected',
    'cancelled' => 'Cancelled',
    'expired' => 'Expired',
    _ => state,
  };

  String _stateHelp(String state) => switch (state) {
    'open' =>
      'Waiting for the other device to join through the short-lived rendezvous.',
    'peerjoined' ||
    'peer_joined' ||
    'awaitingapproval' ||
    'awaiting_approval' => 'Review the peer before accepting the relationship.',
    'approved' =>
      'Your approval was recorded. Waiting for the peer to approve and finish the handshake.',
    'completed' =>
      'The direct peer relationship is ready. The rendezvous is no longer used for normal messages.',
    'rejected' =>
      'This peer was rejected. Create a new invitation to try again.',
    'cancelled' =>
      'Pairing was cancelled. Create or join another invitation when ready.',
    'expired' =>
      'The invitation expired. Create a fresh invitation instead of reusing the old code.',
    _ => 'Pairing is being processed by the secure runtime.',
  };

  String _expiryLabel(int milliseconds) {
    final expiry = DateTime.fromMillisecondsSinceEpoch(milliseconds);
    final remaining = expiry.difference(DateTime.now());
    if (remaining.isNegative) return 'expired';
    final minutes = remaining.inMinutes;
    final seconds = remaining.inSeconds.remainder(60);
    return 'expires in ${minutes}m ${seconds.toString().padLeft(2, '0')}s';
  }
}
