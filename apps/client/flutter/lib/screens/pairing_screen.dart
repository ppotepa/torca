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

/// One place for creating, joining and reviewing invitations.
class PairingScreen extends StatefulWidget {
  const PairingScreen({required this.gateway, super.key});
  final EngineGateway gateway;

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final TextEditingController _code = TextEditingController();
  final OperationTracker _operations = OperationTracker();
  final Set<String> _reviewPromptsShown = <String>{};
  String? _error;
  bool _dialogOpen = false;

  @override
  void initState() {
    super.initState();
    _operations.addListener(_operationChanged);
    widget.gateway.snapshots.addListener(_snapshotChanged);
  }

  @override
  void dispose() {
    _operations.removeListener(_operationChanged);
    widget.gateway.snapshots.removeListener(_snapshotChanged);
    _operations.dispose();
    _code.dispose();
    super.dispose();
  }

  void _operationChanged() {
    if (mounted) setState(() {});
  }

  void _snapshotChanged() {
    if (!mounted || _dialogOpen) return;
    PairingDto? review;
    for (final pairing in widget.gateway.snapshots.value.pairings) {
      if (_needsReview(pairing) && !_reviewPromptsShown.contains(pairing.id)) {
        review = pairing;
        break;
      }
    }
    if (review == null) return;
    _reviewPromptsShown.add(review.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _showSession(review!);
    });
  }

  bool get _busy =>
      _operations.isActive('pairing:create') ||
      _operations.isActive('pairing:join') ||
      _operations.isActive('pairing:scan');

  bool _needsReview(PairingDto pairing) =>
      pairing.role == 'creator' &&
      (pairing.state == 'awaitingapproval' ||
          pairing.state == 'awaiting_approval' ||
          pairing.state == 'peerjoined' ||
          pairing.state == 'peer_joined');

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: const RuntimeAppBar(title: Text('Invitations')),
    body: ValueListenableBuilder<AppSnapshotDto>(
      valueListenable: widget.gateway.snapshots,
      builder: (context, snapshot, _) {
        final relayReady = snapshot.transport.relay.isUsable;
        final review = snapshot.pairings.where(_needsReview).toList().reversed;
        final active = snapshot.pairings
            .where((pairing) => !_needsReview(pairing))
            .toList()
            .reversed;
        return ListView(
          padding: const EdgeInsets.fromLTRB(20, 18, 20, 32),
          children: <Widget>[
            Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 720),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    _PairingIntroCard(relayReady: relayReady),
                    const SizedBox(height: 14),
                    _JoinCard(
                      controller: _code,
                      enabled: relayReady && !_busy,
                      busy: _operations.isActive('pairing:join'),
                      scanBusy: _operations.isActive('pairing:scan'),
                      error: _error,
                      onJoin: _join,
                      onScan: _scan,
                    ),
                    const SizedBox(height: 14),
                    Card(
                      child: Padding(
                        padding: const EdgeInsets.all(16),
                        child: AsyncActionButton(
                          onPressed: relayReady && !_busy ? _create : null,
                          busy: _operations.isActive('pairing:create'),
                          icon: Icons.add_link,
                          label: 'Create invitation',
                        ),
                      ),
                    ),
                    if (!relayReady) ...<Widget>[
                      const SizedBox(height: 14),
                      _RelayBlockedCard(
                        checking: snapshot.transport.relay.state == 'checking',
                      ),
                    ],
                    if (review.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 28),
                      _SectionTitle(
                        title: 'Action required',
                        count: review.length,
                        icon: Icons.verified_user_outlined,
                      ),
                      const SizedBox(height: 8),
                      ...review.map(_sessionTile),
                    ],
                    if (active.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 24),
                      _SectionTitle(
                        title: 'Active invitations',
                        count: active.length,
                        icon: Icons.sync,
                      ),
                      const SizedBox(height: 8),
                      ...active.map(_sessionTile),
                    ],
                    if (snapshot.pairings.isEmpty) ...<Widget>[
                      const SizedBox(height: 24),
                      const _EmptyInvitationsCard(),
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

  Widget _sessionTile(PairingDto pairing) => _PairingSessionCard(
    pairing: pairing,
    busy: _operations.anyWithPrefix('pairing:${pairing.id}:'),
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
  );

  Future<void> _create() async {
    final result = await _run(
      'pairing:create',
      const CreatePairingCommandDto(),
    );
    if (result?.ok != true || !mounted) return;
    final pairings = widget.gateway.snapshots.value.pairings.reversed;
    for (final pairing in pairings) {
      if (pairing.role == 'creator') {
        await _showSession(pairing);
        return;
      }
    }
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
        () => _error = 'Enter a five-character code or scan a Torca QR.',
      );
      return;
    }
    final result = await _run(
      'pairing:join',
      JoinPairingCommandDto(code: code),
    );
    if (result?.ok != true || !mounted) return;
    _code.clear();
    for (final pairing in widget.gateway.snapshots.value.pairings.reversed) {
      if (pairing.role == 'joiner') {
        await _showSession(pairing);
        return;
      }
    }
  }

  Future<void> _showSession(PairingDto pairing) async {
    _dialogOpen = true;
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
              title: completed ? 'Contact added' : 'Invitation closed',
              height: 360,
              child: _TerminalPairingContent(
                completed: completed,
                onClose: () => Navigator.of(context).pop(
                  completed
                      ? 'conversation:${completedConversation!.id}'
                      : 'close',
                ),
              ),
            );
          }
          final session = current;
          return AppModal(
            title: session.role == 'creator'
                ? (_needsReview(session)
                      ? 'Review new contact'
                      : 'Your invitation')
                : 'Your join request',
            height: session.remoteFingerprint == null ? 580 : 650,
            child: _PairingSessionDetails(
              pairing: session,
              busy: _operations.anyWithPrefix('pairing:${session.id}:'),
              onApprove: () => _session(
                session.id,
                'approve',
                ApprovePairingCommandDto(sessionIdHex: session.id),
              ),
              onReject: () => _session(
                session.id,
                'reject',
                RejectPairingCommandDto(sessionIdHex: session.id),
              ),
              onCancel: () => _session(
                session.id,
                'cancel',
                CancelPairingCommandDto(sessionIdHex: session.id),
              ),
            ),
          );
        },
      ),
    );
    _dialogOpen = false;
    if (!mounted ||
        destination == null ||
        !destination.startsWith('conversation:')) {
      return;
    }
    final conversationId = destination.substring('conversation:'.length);
    for (final conversation in widget.gateway.snapshots.value.conversations) {
      if (conversation.id == conversationId) {
        await Navigator.of(context).pushReplacement<void, void>(
          MaterialPageRoute(
            builder: (_) => ConversationScreen(
              gateway: widget.gateway,
              conversation: conversation,
            ),
          ),
        );
        return;
      }
    }
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
      if (scanned != null && mounted) _code.text = _extractCode(scanned) ?? '';
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
    if (RegExp(r'^[0-9A-HJKMNPQRSTVWXYZ]{5}$').hasMatch(direct)) return direct;
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

class _PairingIntroCard extends StatelessWidget {
  const _PairingIntroCard({required this.relayReady});
  final bool relayReady;

  @override
  Widget build(BuildContext context) => Card(
    clipBehavior: Clip.antiAlias,
    child: Container(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          colors: <Color>[
            Theme.of(context).colorScheme.primaryContainer,
            Theme.of(context).colorScheme.surface,
          ],
        ),
      ),
      padding: const EdgeInsets.fromLTRB(20, 20, 20, 18),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          CircleAvatar(
            radius: 24,
            backgroundColor: Theme.of(context).colorScheme.primary,
            foregroundColor: Theme.of(context).colorScheme.onPrimary,
            child: const Icon(Icons.link),
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  'Pair a new contact',
                  style: Theme.of(context).textTheme.titleLarge,
                ),
                const SizedBox(height: 5),
                const Text(
                  'Create an invitation or enter a code. Both devices verify the identity before a private conversation is created.',
                ),
                const SizedBox(height: 10),
                Row(
                  children: <Widget>[
                    Icon(
                      relayReady ? Icons.check_circle : Icons.sync,
                      size: 16,
                      color: relayReady
                          ? Colors.green.shade700
                          : Theme.of(context).colorScheme.outline,
                    ),
                    const SizedBox(width: 6),
                    Text(relayReady ? 'Relay ready' : 'Waiting for relay'),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    ),
  );
}

class _JoinCard extends StatelessWidget {
  const _JoinCard({
    required this.controller,
    required this.enabled,
    required this.busy,
    required this.scanBusy,
    required this.error,
    required this.onJoin,
    required this.onScan,
  });
  final TextEditingController controller;
  final bool enabled, busy, scanBusy;
  final String? error;
  final VoidCallback onJoin, onScan;

  @override
  Widget build(BuildContext context) => Card(
    child: Padding(
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            'Join an invitation',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 4),
          Text(
            'Paste the five-character code or scan the QR shown on the other device.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          const SizedBox(height: 12),
          TextField(
            controller: controller,
            enabled: enabled,
            textCapitalization: TextCapitalization.characters,
            autocorrect: false,
            enableSuggestions: false,
            textInputAction: TextInputAction.join,
            decoration: InputDecoration(
              labelText: 'Invitation code',
              hintText: 'ABC12',
              errorText: error,
              prefixIcon: const Icon(Icons.vpn_key_outlined),
              suffixIcon: IconButton(
                tooltip: 'Scan QR',
                onPressed: enabled ? onScan : null,
                icon: scanBusy
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.qr_code_scanner),
              ),
            ),
            onSubmitted: enabled ? (_) => onJoin() : null,
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: enabled ? onJoin : null,
              icon: busy
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.arrow_forward),
              label: Text(busy ? 'Checking invitation...' : 'Join invitation'),
            ),
          ),
        ],
      ),
    ),
  );
}

class _RelayBlockedCard extends StatelessWidget {
  const _RelayBlockedCard({required this.checking});
  final bool checking;

  @override
  Widget build(BuildContext context) => Card(
    color: Theme.of(context).colorScheme.errorContainer,
    child: ListTile(
      leading: Icon(checking ? Icons.sync : Icons.cloud_off),
      title: Text(
        checking ? 'Verifying secure relay' : 'Secure relay unavailable',
      ),
      subtitle: Text(
        checking
            ? 'Invitations unlock automatically when the Tor connection is verified.'
            : 'Creating and joining invitations are paused until the relay is reachable.',
      ),
    ),
  );
}

class _SectionTitle extends StatelessWidget {
  const _SectionTitle({
    required this.title,
    required this.count,
    required this.icon,
  });
  final String title;
  final int count;
  final IconData icon;

  @override
  Widget build(BuildContext context) => Row(
    children: <Widget>[
      Icon(icon, size: 19),
      const SizedBox(width: 8),
      Text(title, style: Theme.of(context).textTheme.titleMedium),
      const SizedBox(width: 8),
      Badge(label: Text('$count')),
    ],
  );
}

class _EmptyInvitationsCard extends StatelessWidget {
  const _EmptyInvitationsCard();

  @override
  Widget build(BuildContext context) => Card(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        children: <Widget>[
          Icon(
            Icons.people_outline,
            size: 38,
            color: Theme.of(context).colorScheme.outline,
          ),
          const SizedBox(height: 10),
          const Text('No active invitations'),
          const SizedBox(height: 4),
          Text(
            'Create one and share the code, or enter a code you received.',
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ),
    ),
  );
}

class _PairingSessionCard extends StatelessWidget {
  const _PairingSessionCard({
    required this.pairing,
    required this.busy,
    required this.onOpen,
    required this.onApprove,
    required this.onReject,
    required this.onCancel,
  });
  final PairingDto pairing;
  final bool busy;
  final VoidCallback onOpen, onApprove, onReject, onCancel;

  @override
  Widget build(BuildContext context) => Card(
    margin: const EdgeInsets.only(bottom: 9),
    child: ListTile(
      onTap: onOpen,
      leading: CircleAvatar(
        child: Icon(pairing.role == 'creator' ? Icons.qr_code_2 : Icons.link),
      ),
      title: Text(
        pairing.remoteFingerprint == null
            ? (pairing.role == 'creator'
                  ? 'Sent invitation'
                  : 'Joined invitation')
            : 'New contact to review',
      ),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(_displayCode(pairing.code)),
          Text(
            '${_stateLabel(pairing.state)}  ·  ${_expiryLabel(pairing.expiresAtMs)}',
          ),
        ],
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (busy)
            const Padding(
              padding: EdgeInsets.only(right: 8),
              child: SizedBox(
                width: 17,
                height: 17,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
            ),
          Chip(
            label: Text(_stateLabel(pairing.state)),
            backgroundColor: _stateColor(context, pairing.state).withAlpha(35),
          ),
          const Icon(Icons.chevron_right),
        ],
      ),
    ),
  );

  static String _stateLabel(String state) => switch (state) {
    'open' => 'Waiting',
    'peerjoined' || 'peer_joined' => 'Review',
    'awaitingapproval' || 'awaiting_approval' => 'Review',
    'approved' => 'Approved',
    'completed' => 'Connected',
    'rejected' => 'Rejected',
    'cancelled' => 'Cancelled',
    'expired' => 'Expired',
    _ => state,
  };

  static String _displayCode(String code) {
    if (code.length == 5) return '${code.substring(0, 3)}-${code.substring(3)}';
    if (code.length == 6) return '${code.substring(0, 3)}-${code.substring(3)}';
    return code;
  }

  static Color _stateColor(BuildContext context, String state) =>
      switch (state) {
        'awaitingapproval' ||
        'awaiting_approval' ||
        'peerjoined' ||
        'peer_joined' => Colors.orange.shade800,
        'approved' || 'completed' => Colors.green.shade700,
        'rejected' ||
        'cancelled' ||
        'expired' => Theme.of(context).colorScheme.error,
        _ => Theme.of(context).colorScheme.primary,
      };

  static String _expiryLabel(int milliseconds) {
    final remaining = DateTime.fromMillisecondsSinceEpoch(
      milliseconds,
    ).difference(DateTime.now());
    if (remaining.isNegative) return 'expired';
    return 'expires in ${remaining.inMinutes}m ${remaining.inSeconds.remainder(60).toString().padLeft(2, '0')}s';
  }
}

class _PairingSessionDetails extends StatelessWidget {
  const _PairingSessionDetails({
    required this.pairing,
    required this.busy,
    required this.onApprove,
    required this.onReject,
    required this.onCancel,
  });
  final PairingDto pairing;
  final bool busy;
  final VoidCallback onApprove, onReject, onCancel;

  bool get _canReview =>
      pairing.role == 'creator' &&
      (pairing.state == 'awaitingapproval' ||
          pairing.state == 'awaiting_approval' ||
          pairing.state == 'peerjoined' ||
          pairing.state == 'peer_joined');

  String get _uri =>
      'torca://pair?v=1&code=${Uri.encodeQueryComponent(pairing.code)}';

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: <Widget>[
      _StatusBanner(pairing: pairing),
      const SizedBox(height: 14),
      if (pairing.remoteFingerprint != null) ...<Widget>[
        Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(14),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                'Remote identity',
                style: Theme.of(context).textTheme.titleSmall,
              ),
              const SizedBox(height: 8),
              SelectableText(
                pairing.remoteFingerprint!,
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  fontFamily: 'monospace',
                  letterSpacing: 1.1,
                ),
              ),
              if (pairing.remoteIdentityId != null) ...<Widget>[
                const SizedBox(height: 5),
                Text(
                  'Identity ${pairing.remoteIdentityId}',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ],
          ),
        ),
        const SizedBox(height: 14),
      ],
      PairingProgress(state: pairing.state),
      const SizedBox(height: 12),
      if (pairing.role == 'creator' && pairing.state == 'open') ...<Widget>[
        _QrInvitationCard(uri: _uri, code: pairing.code),
        const SizedBox(height: 12),
      ],
      if (_canReview) ...<Widget>[
        Text(
          'A device joined this invitation. Verify the fingerprint before accepting the contact.',
          style: Theme.of(context).textTheme.bodyMedium,
        ),
        const SizedBox(height: 14),
        Row(
          children: <Widget>[
            Expanded(
              child: FilledButton.icon(
                onPressed: busy ? null : onApprove,
                icon: const Icon(Icons.check),
                label: const Text('Accept'),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: OutlinedButton.icon(
                onPressed: busy ? null : onReject,
                icon: const Icon(Icons.close),
                label: const Text('Reject'),
              ),
            ),
          ],
        ),
      ] else if (pairing.role == 'joiner' && !_canReview) ...<Widget>[
        const Text(
          'Your request is waiting for the invitation owner to verify and accept it.',
        ),
        const SizedBox(height: 12),
        OutlinedButton.icon(
          onPressed: busy ? null : onCancel,
          icon: const Icon(Icons.cancel_outlined),
          label: const Text('Cancel request'),
        ),
      ] else if (pairing.state == 'open') ...<Widget>[
        const Text(
          'Share this invitation with the other device before it expires.',
        ),
        const SizedBox(height: 12),
        OutlinedButton.icon(
          onPressed: busy ? null : onCancel,
          icon: const Icon(Icons.cancel_outlined),
          label: const Text('Cancel invitation'),
        ),
      ],
    ],
  );
}

class _StatusBanner extends StatelessWidget {
  const _StatusBanner({required this.pairing});
  final PairingDto pairing;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(14),
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.primaryContainer,
      borderRadius: BorderRadius.circular(14),
    ),
    child: Row(
      children: <Widget>[
        const Icon(Icons.shield_outlined),
        const SizedBox(width: 10),
        Expanded(
          child: Text(
            pairing.state == 'open'
                ? 'Invitation is active for five minutes.'
                : pairing.remoteFingerprint != null
                ? 'Identity received. Review it before accepting.'
                : 'Pairing is being synchronized securely.',
          ),
        ),
      ],
    ),
  );
}

class _QrInvitationCard extends StatelessWidget {
  const _QrInvitationCard({required this.uri, required this.code});
  final String uri, code;

  @override
  Widget build(BuildContext context) => Column(
    children: <Widget>[
      Container(
        color: Colors.white,
        padding: const EdgeInsets.all(18),
        child: QrImageView(
          data: uri,
          size: 190,
          backgroundColor: Colors.white,
          semanticsLabel: 'Torca pairing invitation QR code',
        ),
      ),
      const SizedBox(height: 10),
      SelectableText(
        code.length == 5
            ? '${code.substring(0, 3)}-${code.substring(3)}'
            : code,
        style: Theme.of(context).textTheme.headlineSmall?.copyWith(
          fontFamily: 'monospace',
          letterSpacing: 3,
        ),
      ),
      const SizedBox(height: 8),
      OutlinedButton.icon(
        onPressed: () async {
          await Clipboard.setData(ClipboardData(text: code));
          if (context.mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(content: Text('Invitation code copied')),
            );
          }
        },
        icon: const Icon(Icons.copy_outlined),
        label: const Text('Copy code'),
      ),
    ],
  );
}

class _TerminalPairingContent extends StatelessWidget {
  const _TerminalPairingContent({
    required this.completed,
    required this.onClose,
  });
  final bool completed;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: <Widget>[
      Icon(
        completed ? Icons.mark_chat_unread_outlined : Icons.info_outline,
        size: 52,
        color: completed ? Colors.green.shade700 : null,
      ),
      const SizedBox(height: 14),
      Text(
        completed
            ? 'The contact was added securely. Open the private conversation now.'
            : 'This invitation is no longer active. The other device will receive the same final state.',
        textAlign: TextAlign.center,
      ),
      const SizedBox(height: 20),
      FilledButton.icon(
        onPressed: onClose,
        icon: Icon(completed ? Icons.forum_outlined : Icons.close),
        label: Text(completed ? 'Open conversation' : 'Close'),
      ),
    ],
  );
}
