import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/app_modal.dart';
import '../widgets/async_action_button.dart';
import '../widgets/operation_tracker.dart';
import '../widgets/pairing_progress.dart';
import '../widgets/runtime_network_status.dart';
import 'conversation_screen.dart';

/// Opens one invitation in place without navigating away from the current screen.
Future<void> showPairingSessionModal(
  BuildContext context,
  EngineGateway gateway,
  PairingDto pairing,
) async {
await showDialog<void>(
context: context,
requestFocus: true,
builder: (_) => _PairingSessionModal(gateway: gateway, pairing: pairing),
);
}

/// Opens the creator flow from the Invitations section. Joining is deliberately
/// absent: it belongs to the add-contact flow.
Future<void> showInvitationGeneratorModal(
  BuildContext context,
  EngineGateway gateway,
) => showDialog<void>(
  context: context,
  requestFocus: true,
  builder: (_) => _PairingComposerModal(
    gateway: gateway,
    mode: _PairingComposerMode.create,
  ),
);

/// Opens only the join flow used by Contacts and the global add-contact action.
Future<void> showJoinInvitationModal(
  BuildContext context,
  EngineGateway gateway, {
  String? initialCode,
}) => showDialog<void>(
  context: context,
  requestFocus: true,
  builder: (_) => _PairingComposerModal(
    gateway: gateway,
    mode: _PairingComposerMode.join,
    initialCode: initialCode,
  ),
);

enum _PairingComposerMode { create, join }

class _PairingComposerModal extends StatefulWidget {
  const _PairingComposerModal({
    required this.gateway,
    required this.mode,
    this.initialCode,
  });

  final EngineGateway gateway;
  final _PairingComposerMode mode;
  final String? initialCode;

  @override
  State<_PairingComposerModal> createState() => _PairingComposerModalState();
}

class _PairingComposerModalState extends State<_PairingComposerModal> {
  final _code = TextEditingController();
  final _focus = FocusNode(debugLabel: 'pairing-composer-code');
  final _operations = OperationTracker();
  String? _error;
  PairingDto? _createdPairing;
  String? _inviteUri;
  String? _createdSessionId;

  @override
  void initState() {
    super.initState();
    _operations.addListener(_changed);
    if (widget.mode == _PairingComposerMode.create) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _create());
    } else {
      _code.text = widget.initialCode ?? '';
      WidgetsBinding.instance.addPostFrameCallback((_) => _focusCodeInput());
    }
  }

  @override
  void dispose() {
    _operations
      ..removeListener(_changed)
      ..dispose();
    _focus.dispose();
    _code.dispose();
    super.dispose();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  void _focusCodeInput() {
    if (!mounted) return;
    // Let Flutter own the IME connection. Calling the private text-input
    // channel from a modal route can make Android show the keyboard while the
    // EditableText connection is already stale, so keystrokes disappear.
    // Android may attach the dialog route one frame after the modal widget is
    // built. Requesting focus twice (on the next frame and after a short
    // scheduler turn) makes the EditableText/IME connection deterministic
    // without touching the private text-input channel.
    _focus.requestFocus();
    Future<void>.delayed(const Duration(milliseconds: 80), () {
      if (mounted && !_focus.hasFocus) _focus.requestFocus();
    });
  }

  Future<void> _create() async {
    final result = await _run(
      'pairing:create',
      const CreatePairingCommandDto(),
    );
    if (result == null || !result.ok || !mounted) return;
    if (result.kind == 'pairing_queued') {
      // A queued create has no invitation URI/QR yet. Keeping the modal open
      // would trap the user behind an indefinite placeholder and make the
      // Contacts/Chats tabs appear broken. The pending operation remains in
      // the snapshot and will produce the invitation once NETWORK_READY is
      // reached.
      final messenger = ScaffoldMessenger.of(context);
      Navigator.of(context).pop();
      messenger.showSnackBar(
        const SnackBar(
          content: Text(
            'Invitation queued. It will be generated when the secure network is ready.',
          ),
        ),
      );
      return;
    }
    setState(() {
      _createdSessionId = result.resourceId;
      _createdPairing = _pairingFor(result.resourceId);
      _inviteUri = result.inviteUri;
    });
  }

  Future<void> _scan() async {
    await _operations.run('pairing:scan', () async {
      final scanned = await showDialog<String>(
        context: context,
        builder: (dialogContext) => Dialog(
          child: SizedBox(
            width: 420,
            height: 520,
            child: Stack(
              children: <Widget>[
                MobileScanner(
                  onDetect: (capture) {
                    for (final barcode in capture.barcodes) {
                      final value = barcode.rawValue;
                      if (value != null && value.trim().isNotEmpty) {
                        Navigator.of(dialogContext).pop(value);
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
                    onPressed: () => Navigator.of(dialogContext).pop(),
                    icon: Icon(context.torcaIcons.close),
                  ),
                ),
              ],
            ),
          ),
        ),
      );
      if (scanned != null && mounted) _code.text = scanned;
    });
    if (_code.text.isNotEmpty && mounted) await _join();
  }

  PairingDto? _currentCreated(AppSnapshotDto snapshot) {
    final id = _createdSessionId;
    if (id == null) return _createdPairing;
    for (final pairing in snapshot.pairings) {
      if (pairing.id == id) return pairing;
    }
    return _createdPairing;
  }

  Future<void> _join() async {
    final raw = _code.text.trim();
    final parser = widget.gateway is PairingUriParser
        ? widget.gateway as PairingUriParser
        : null;
    final code = parser == null ? raw : await parser.parsePairingUri(raw);
    if (code == null) {
      setState(() => _error = 'Enter a six-character code or scan a Torca QR.');
      return;
    }
    final ticket = RegExp(
      r'[?&]ticket=([0-9a-fA-F]{32})',
    ).firstMatch(raw)?.group(1)?.toLowerCase();
    final result = await _run(
      'pairing:join',
      JoinPairingCommandDto(code: code, ticket: ticket),
    );
    if (result?.ok != true || !mounted) return;
    _code.clear();
    final messenger = ScaffoldMessenger.of(context);
    Navigator.of(context).pop();
    messenger.showSnackBar(
      SnackBar(
        content: Text(
          result!.kind == 'pairing_queued'
              ? 'Saved locally. It will be sent when your private endpoint is ready.'
              : 'Join request sent. You will be notified when it is accepted.',
        ),
      ),
    );
  }

  PairingDto? _pairingFor(String? id) => widget.gateway.snapshots.value.pairings
      .where((pairing) => pairing.id == id)
      .firstOrNull;

  Future<BridgeResultDto?> _run(String key, BridgeCommandDto command) async {
    BridgeResultDto? result;
    await _operations.run(key, () async {
      setState(() => _error = null);
      result = await widget.gateway.execute(command);
      if (result?.ok != true && mounted) {
        setState(() => _error = result?.error ?? 'Invitation operation failed');
      }
    });
    return result;
  }

  @override
  Widget build(BuildContext context) => AppModal(
    title: widget.mode == _PairingComposerMode.create
        ? 'Your invitation'
        : 'Join invitation',
    height: widget.mode == _PairingComposerMode.create ? 620 : 360,
    scrollable: widget.mode == _PairingComposerMode.create,
    child: widget.mode == _PairingComposerMode.create
        ? ValueListenableBuilder<AppSnapshotDto>(
            valueListenable: widget.gateway.snapshots,
            builder: (context, snapshot, _) {
              final pairing = _currentCreated(snapshot);
              if (pairing != null) {
                return _PairingSessionDetails(
                  pairing: pairing,
                  inviteUri: _inviteUri,
                  busy: _operations.anyWithPrefix('pairing:${pairing.id}:'),
                  onApprove: () => _run(
                    'pairing:${pairing.id}:approve',
                    ApprovePairingCommandDto(sessionIdHex: pairing.id),
                  ),
                  onReject: () => _run(
                    'pairing:${pairing.id}:reject',
                    RejectPairingCommandDto(sessionIdHex: pairing.id),
                  ),
                  onCancel: () => _run(
                    'pairing:${pairing.id}:cancel',
                    CancelPairingCommandDto(sessionIdHex: pairing.id),
                  ),
                );
              }
              return _InvitationGenerationPlaceholder(
                busy: _operations.isActive('pairing:create'),
                error: _error,
                onRetry: _operations.isActive('pairing:create')
                    ? null
                    : _create,
              );
            },
          )
        : _JoinCard(
            controller: _code,
            focusNode: _focus,
            enabled: !_operations.isActive('pairing:join'),
            scanEnabled:
                !kIsWeb && defaultTargetPlatform == TargetPlatform.android,
            busy: _operations.isActive('pairing:join'),
            scanBusy: _operations.isActive('pairing:scan'),
            error: _error,
            onJoin: _join,
            onScan: _scan,
            onFocusInput: _focusCodeInput,
          ),
  );
}

class _InvitationGenerationPlaceholder extends StatelessWidget {
  const _InvitationGenerationPlaceholder({
    required this.busy,
    required this.error,
    required this.onRetry,
  });

  final bool busy;
  final String? error;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.surfaceContainerHighest;
    final glow = Theme.of(context).colorScheme.primary.withAlpha(45);
    Widget block({double? width, required double height}) => Container(
      width: width,
      height: height,
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(context.torcaTokens.radiusSmall),
        boxShadow: <BoxShadow>[
          BoxShadow(color: glow, blurRadius: 12, spreadRadius: 1),
        ],
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          busy
              ? 'Generating a private invitation…'
              : 'Invitation is waiting for the network.',
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 16),
        Center(child: block(width: 140, height: 140)),
        const SizedBox(height: 18),
        Center(child: block(width: 178, height: 28)),
        const SizedBox(height: 12),
        Center(child: block(width: 120, height: 18)),
        const Spacer(),
        if (error != null) ...<Widget>[
          Text(
            error!,
            textAlign: TextAlign.center,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
          const SizedBox(height: 10),
        ],
        OutlinedButton.icon(
          onPressed: onRetry,
          icon: Icon(context.torcaIcons.reconnect),
          label: Text(busy ? 'Generating…' : 'Retry generation'),
        ),
      ],
    );
  }
}

class _PairingSessionModal extends StatefulWidget {
  const _PairingSessionModal({required this.gateway, required this.pairing});
  final EngineGateway gateway;
  final PairingDto pairing;

  @override
  State<_PairingSessionModal> createState() => _PairingSessionModalState();
}

class _PairingSessionModalState extends State<_PairingSessionModal> {
  bool _busy = false;
  String? _error;

  PairingDto? _current(AppSnapshotDto snapshot) {
    for (final pairing in snapshot.pairings) {
      if (pairing.id == widget.pairing.id) return pairing;
    }
    return null;
  }

  Future<void> _run(BridgeCommandDto command) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final result = await widget.gateway.execute(command);
    if (!mounted) return;
    if (!result.ok) {
      setState(() {
        _busy = false;
        _error = result.error ?? 'Pairing operation failed';
      });
      return;
    }
    setState(() => _busy = false);
  }

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: widget.gateway.snapshots,
    builder: (context, snapshot, _) {
      final pairing = _current(snapshot);
      return AppModal(
        title: pairing?.typedRole == PairingRole.creator
            ? 'Invitation'
            : 'Join request',
        height: 500,
        scrollable: false,
        child: pairing == null
            ? _TerminalPairingContent(
                completed: widget.pairing.typedState == PairingState.approved,
                onClose: () => Navigator.of(context).pop(),
              )
            : Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  if (_error != null) ...<Widget>[
                    Text(
                      _error!,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                    const SizedBox(height: 10),
                  ],
                  _PairingSessionDetails(
                    pairing: pairing,
                    busy: _busy,
                    onApprove: () => _run(
                      ApprovePairingCommandDto(sessionIdHex: pairing.id),
                    ),
                    onReject: () =>
                        _run(RejectPairingCommandDto(sessionIdHex: pairing.id)),
                    onCancel: () =>
                        _run(CancelPairingCommandDto(sessionIdHex: pairing.id)),
                  ),
                ],
              ),
      );
    },
  );
}

/// One place for creating, joining and reviewing invitations.
class PairingScreen extends StatefulWidget {
  const PairingScreen({required this.gateway, super.key});
  final EngineGateway gateway;

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final OperationTracker _operations = OperationTracker();

  @override
  void initState() {
    super.initState();
    _operations.addListener(_operationChanged);
  }

  @override
  void dispose() {
    _operations.removeListener(_operationChanged);
    _operations.dispose();
    super.dispose();
  }

  void _operationChanged() {
    if (mounted) setState(() {});
  }

  bool _needsReview(PairingDto pairing) =>
      pairing.typedRole == PairingRole.creator &&
      (pairing.typedState == PairingState.awaitingApproval ||
          pairing.typedState == PairingState.peerJoined);

  bool _isVisible(PairingDto pairing) {
    if (const {
      PairingState.rejected,
      PairingState.cancelled,
      PairingState.expired,
      PairingState.completed,
      PairingState.approved,
    }.contains(pairing.typedState)) {
      return false;
    }
    return pairing.expiresAtMs > DateTime.now().millisecondsSinceEpoch ||
        pairing.typedState == PairingState.approved;
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: const RuntimeAppBar(title: Text('Invitations')),
    body: ValueListenableBuilder<AppSnapshotDto>(
      valueListenable: widget.gateway.snapshots,
      builder: (context, snapshot, _) {
        final relayReady = snapshot.transport.relay.isUsable;
        final visible = snapshot.pairings.where(_isVisible).toList();
        final pending = snapshot.pendingOperations
            .where((operation) => operation.kind.startsWith('pairing.'))
            .toList(growable: false);
        final review = visible.where(_needsReview).toList().reversed;
        final active = visible
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
                    Card(
                      child: Padding(
                        padding: const EdgeInsets.all(16),
                        child: AsyncActionButton(
                          onPressed: () => showInvitationGeneratorModal(
                            context,
                            widget.gateway,
                          ),
                          busy: false,
                          icon: context.torcaIcons.invitations,
                          label: 'Generate Invitation',
                        ),
                      ),
                    ),
                    if (!relayReady) ...<Widget>[
                      const SizedBox(height: 14),
                      _RelayBlockedCard(
                        checking: snapshot.transport.relay.state == 'checking',
                      ),
                    ],
                    if (pending.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 20),
                      _SectionTitle(
                        title: 'Waiting for network',
                        count: pending.length,
                        icon: context.torcaIcons.reconnect,
                      ),
                      const SizedBox(height: 8),
                      ...pending.map(
                        (operation) => _PendingPairingCard(
                          operation,
                          onCancel: operation.kind == 'pairing.create' ||
                                  operation.kind == 'pairing.join'
                              ? () => _session(
                                    operation.resourceId,
                                    'cancel',
                                    CancelPairingCommandDto(
                                      sessionIdHex: operation.resourceId,
                                    ),
                                  )
                              : null,
                        ),
                      ),
                    ],
                    if (review.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 28),
                      _SectionTitle(
                        title: 'Action required',
                        count: review.length,
                        icon: context.torcaIcons.identity,
                      ),
                      const SizedBox(height: 8),
                      ...review.map(_sessionTile),
                    ],
                    if (active.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 24),
                      _SectionTitle(
                        title: 'Active invitations',
                        count: active.length,
                        icon: context.torcaIcons.reconnect,
                      ),
                      const SizedBox(height: 8),
                      ...active.map(_sessionTile),
                    ],
                    if (visible.isEmpty) ...<Widget>[
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

  Future<void> _showSession(PairingDto pairing, {String? inviteUri}) async {
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
            return AppModal(
              title: 'Invitation closed',
              height: 360,
              scrollable: false,
              child: _TerminalPairingContent(
                completed: false,
                onClose: () => Navigator.of(context).pop('close'),
              ),
            );
          }
          final session = current;
          return AppModal(
            title: session.typedRole == PairingRole.creator
                ? (_needsReview(session)
                      ? 'Review new contact'
                      : 'Your invitation')
                : 'Your join request',
            height: session.remoteFingerprint == null ? 500 : 520,
            scrollable: false,
            child: _PairingSessionDetails(
              pairing: session,
              inviteUri: inviteUri,
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

  Future<BridgeResultDto?> _run(String key, BridgeCommandDto command) async {
    BridgeResultDto? result;
    await _operations.run(key, () async {
      result = await widget.gateway.execute(command);
      if (mounted && result?.ok == false) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(result?.error ?? 'Pairing operation failed')),
        );
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
}

class _PairingIntroCard extends StatelessWidget {
  const _PairingIntroCard({required this.relayReady});
  final bool relayReady;

  @override
  Widget build(BuildContext context) => Card(
    clipBehavior: Clip.antiAlias,
    child: Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerLow,
      ),
      padding: const EdgeInsets.fromLTRB(20, 20, 20, 18),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          TorcaAvatar(
            label: 'Pairing',
            size: 48,
            backgroundColor: Theme.of(context).colorScheme.primary,
            foregroundColor: Theme.of(context).colorScheme.onPrimary,
            child: Icon(context.torcaIcons.link),
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  'Private invitations',
                  style: Theme.of(context).textTheme.titleLarge,
                ),
                const SizedBox(height: 5),
                const Text(
                  'Generate a short-lived code for the other device. Joining a received code is available from Contacts.',
                ),
                const SizedBox(height: 10),
                Row(
                  children: <Widget>[
                    Icon(
                      relayReady
                          ? context.torcaIcons.success
                          : context.torcaIcons.reconnect,
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
    required this.focusNode,
    required this.enabled,
    required this.scanEnabled,
    required this.busy,
    required this.scanBusy,
    required this.error,
    required this.onJoin,
    required this.onScan,
    required this.onFocusInput,
  });
  final TextEditingController controller;
  final FocusNode focusNode;
  final bool enabled, scanEnabled, busy, scanBusy;
  final String? error;
  final VoidCallback onJoin, onScan, onFocusInput;

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
            scanEnabled || scanBusy
                ? 'Paste the six-character code or scan the QR shown on the other device.'
                : 'Paste the six-character code from the other device.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          const SizedBox(height: 12),
          TextField(
            controller: controller,
            focusNode: focusNode,
            autofocus: true,
            enabled: enabled,
            keyboardType: TextInputType.text,
            textCapitalization: TextCapitalization.characters,
            autocorrect: false,
            enableSuggestions: false,
            // Keep the editable value untouched.  Filtering formatters can
            // reject Android composing-text updates (the keyboard is visible
            // but committed characters never reach the controller).  The
            // typed-code/URI parser performs normalization and validation
            // after editing, so pasted QR URIs remain supported as well.
            textInputAction: TextInputAction.join,
            decoration: InputDecoration(
              labelText: 'Invitation code',
              hintText: 'ABC123',
              errorText: error,
              prefixIcon: Icon(context.torcaIcons.identity),
              suffixIcon: scanEnabled || scanBusy
                  ? IconButton(
                      tooltip: 'Scan QR',
                      onPressed: scanEnabled ? onScan : null,
                      icon: scanBusy
                          ? const SizedBox(
                              width: 18,
                              height: 18,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : Icon(context.torcaIcons.scan),
                    )
                  : null,
            ),
            onTap: onFocusInput,
            onSubmitted: enabled ? (_) => onJoin() : null,
          ),
          const SizedBox(height: 12),
          SizedBox(
            width: double.infinity,
            child: FilledButton.icon(
              onPressed: enabled ? onJoin : null,
              icon: busy
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Icon(context.torcaIcons.send),
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
      leading: Icon(
        checking ? context.torcaIcons.reconnect : context.torcaIcons.error,
      ),
      title: Text(
        checking ? 'Connecting to secure relay' : 'Secure relay unavailable',
      ),
      subtitle: Text(
        checking
            ? 'Relay verification is running. You can still enter a code; the request will report the authoritative result.'
            : 'Creating invitations is paused. You can still enter a code to retry the relay directly.',
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
      TorcaBadge(label: Text('$count')),
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
            context.torcaIcons.contacts,
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

class _PendingPairingCard extends StatelessWidget {
  const _PendingPairingCard(this.operation, {this.onCancel});

  final PendingOperationDto operation;
  final VoidCallback? onCancel;

  String get _dependencyLabel => switch (operation.dependency) {
    'tor_onion_and_relay' => 'Tor, private endpoint and relay',
    'tor' => 'Tor network',
    'relay' => 'secure relay',
    _ => operation.dependency,
  };

  @override
  Widget build(BuildContext context) {
    final action = switch (operation.kind) {
      'pairing.create' => 'Generating invitation',
      'pairing.join' => 'Joining invitation',
      'pairing.approve' => 'Accepting contact',
      'pairing.reject' => 'Rejecting request',
      'pairing.cancel' => 'Cancelling invitation',
      _ => 'Pairing operation',
    };
    return Card(
      child: ListTile(
        leading: const SizedBox.square(
          dimension: 22,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
        title: Text(action),
        subtitle: Text(
          operation.attempts == 0
              ? 'Waiting for $_dependencyLabel'
              : 'Retry ${operation.attempts} · waiting for $_dependencyLabel',
        ),
        trailing: onCancel == null
            ? Text(operation.state)
            : IconButton(
                tooltip: 'Cancel operation',
                onPressed: onCancel,
                icon: Icon(context.torcaIcons.close),
              ),
      ),
    );
  }
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
      leading: TorcaAvatar(
        label: pairing.role,
        child: Icon(
          pairing.role == 'creator'
              ? context.torcaIcons.invitations
              : context.torcaIcons.link,
        ),
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
          Icon(context.torcaIcons.expand),
        ],
      ),
    ),
  );

  static String _stateLabel(String state) => switch (state) {
    'open' => 'Waiting',
    'peerjoined' => 'Review',
    'awaitingapproval' => 'Review',
    'approved' => 'Approved',
    'completed' => 'Connected',
    'rejected' => 'Rejected',
    'cancelled' => 'Cancelled',
    'expired' => 'Expired',
    _ => state,
  };

  static String _displayCode(String code) {
    return code.replaceAll(RegExp(r'\s+'), '').toUpperCase();
  }

  static Color _stateColor(BuildContext context, String state) =>
      switch (state) {
        'awaitingapproval' || 'peerjoined' => Colors.orange.shade800,
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
    this.inviteUri,
    required this.busy,
    required this.onApprove,
    required this.onReject,
    required this.onCancel,
  });
  final PairingDto pairing;
  final String? inviteUri;
  final bool busy;
  final VoidCallback onApprove, onReject, onCancel;

  bool get _canReview =>
      pairing.typedRole == PairingRole.creator &&
      (pairing.typedState == PairingState.awaitingApproval ||
          pairing.typedState == PairingState.peerJoined);

  String get _uri => inviteUri ?? pairing.inviteUri;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: <Widget>[
      if (pairing.remoteFingerprint != null) ...<Widget>[
        Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(
              context.torcaTokens.radiusLarge,
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                'Remote identity',
                style: Theme.of(context).textTheme.titleSmall,
              ),
              if (pairing.remoteDisplayName != null &&
                  pairing.remoteDisplayName!.trim().isNotEmpty) ...<Widget>[
                const SizedBox(height: 5),
                Text(
                  pairing.remoteDisplayName!,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ],
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
        _QrInvitationCard(
          uri: _uri,
          code: pairing.code,
          expiresAtMs: pairing.expiresAtMs,
        ),
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
                icon: Icon(context.torcaIcons.confirm),
                label: const Text('Accept'),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: OutlinedButton.icon(
                onPressed: busy ? null : onReject,
                icon: Icon(context.torcaIcons.close),
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
          style: OutlinedButton.styleFrom(
            foregroundColor: Theme.of(context).colorScheme.error,
            side: BorderSide(color: Theme.of(context).colorScheme.error),
          ),
          icon: Icon(context.torcaIcons.cancelled),
          label: const Text('Cancel request'),
        ),
      ] else if (pairing.state == 'open') ...<Widget>[
        OutlinedButton.icon(
          onPressed: busy ? null : onCancel,
          style: OutlinedButton.styleFrom(
            foregroundColor: Theme.of(context).colorScheme.error,
            side: BorderSide(color: Theme.of(context).colorScheme.error),
          ),
          icon: Icon(context.torcaIcons.cancelled),
          label: const Text('Cancel invitation'),
        ),
      ],
    ],
  );
}

class _QrInvitationCard extends StatefulWidget {
  const _QrInvitationCard({
    required this.uri,
    required this.code,
    required this.expiresAtMs,
  });
  final String uri, code;
  final int expiresAtMs;

  @override
  State<_QrInvitationCard> createState() => _QrInvitationCardState();
}

class _QrInvitationCardState extends State<_QrInvitationCard> {
  Timer? _timer;
  late Duration _remaining;

  @override
  void initState() {
    super.initState();
    _remaining = _untilExpiry();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) setState(() => _remaining = _untilExpiry());
    });
  }

  Duration _untilExpiry() => DateTime.fromMillisecondsSinceEpoch(
    widget.expiresAtMs,
  ).difference(DateTime.now());

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  String get _countdown {
    if (_remaining.isNegative) return 'Expired';
    final minutes = _remaining.inMinutes.toString().padLeft(2, '0');
    final seconds = _remaining.inSeconds.remainder(60).toString().padLeft(2, '0');
    return 'Expires in $minutes:$seconds';
  }

  @override
  Widget build(BuildContext context) {
    final expired = _remaining.isNegative;
    final warning = !expired && _remaining.inSeconds <= 60;
    final scheme = Theme.of(context).colorScheme;
    final displayCode = widget.code.replaceAll(RegExp(r'\s+'), '').toUpperCase();
    return Column(
      children: <Widget>[
        Container(
          color: Colors.white,
          padding: const EdgeInsets.all(12),
          child: QrImageView(
            data: widget.uri,
            size: 200,
            backgroundColor: Colors.white,
            semanticsLabel: 'Torca pairing invitation QR code',
          ),
        ),
        const SizedBox(height: 12),
        SelectableText(
          displayCode,
          style: Theme.of(context).textTheme.headlineMedium?.copyWith(
            fontFamily: 'monospace',
            fontWeight: FontWeight.w700,
            letterSpacing: 4,
          ),
        ),
        const SizedBox(height: 6),
        Text(
          _countdown,
          style: Theme.of(context).textTheme.titleSmall?.copyWith(
            color: expired || warning ? scheme.error : scheme.primary,
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 10),
        SizedBox(
          width: double.infinity,
          child: FilledButton.icon(
            onPressed: expired
                ? null
                : () async {
                    await Clipboard.setData(ClipboardData(text: widget.code));
                    if (context.mounted) {
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(content: Text('Invitation code copied')),
                      );
                    }
                  },
            icon: Icon(context.torcaIcons.copy),
            label: const Text('Copy code'),
          ),
        ),
      ],
    );
  }
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
        completed ? context.torcaIcons.chats : context.torcaIcons.info,
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
        icon: Icon(
          completed ? context.torcaIcons.chats : context.torcaIcons.close,
        ),
        label: Text(completed ? 'Open conversation' : 'Close'),
      ),
    ],
  );
}
