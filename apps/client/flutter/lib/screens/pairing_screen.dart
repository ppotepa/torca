import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:torca_ui/torca_ui.dart';

import '../controllers/pairing_action_controller.dart';
import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
import '../widgets/app_modal.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/operation_tracker.dart';
import '../widgets/pairing_modal_registry.dart';
import '../widgets/pairing_progress.dart';

/// Opens one invitation in place without navigating away from the current screen.
Future<void> showPairingSessionModal(
  BuildContext context,
  EngineGateway gateway,
  PairingDto pairing,
) async {
  final registry = PairingModalRegistry.instance;
  if (registry.owns(pairing.id)) return;
  registry.claim(pairing.id);
  try {
    await showDialog<void>(
      context: context,
      barrierDismissible: true,
      requestFocus: true,
      builder: (_) => _PairingSessionModal(gateway: gateway, pairing: pairing),
    );
  } finally {
    registry.release(pairing.id);
  }
}

/// Opens the creator flow from the Invitations section. Joining is deliberately
/// absent: it belongs to the add-contact flow.
Future<void> showInvitationGeneratorModal(
  BuildContext context,
  EngineGateway gateway,
) => showDialog<void>(
  context: context,
  barrierDismissible: true,
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
  barrierDismissible: true,
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
  bool _queued = false;
  bool _completionCloseScheduled = false;
  Set<String> _contactsBeforeCreation = const <String>{};

  @override
  void initState() {
    super.initState();
    _operations.addListener(_changed);
    widget.gateway.snapshots.addListener(_snapshotChanged);
    if (widget.mode == _PairingComposerMode.create) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _create());
    } else {
      _code.text = widget.initialCode ?? '';
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => _focusCodeInput(reconnect: true),
      );
    }
  }

  @override
  void dispose() {
    if (widget.mode == _PairingComposerMode.create) {
      PairingModalRegistry.instance.release(_createdSessionId);
    }
    _operations
      ..removeListener(_changed)
      ..dispose();
    widget.gateway.snapshots.removeListener(_snapshotChanged);
    _focus.dispose();
    _code.dispose();
    super.dispose();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  void _snapshotChanged() {
    if (!mounted || widget.mode != _PairingComposerMode.create) return;
    final snapshot = widget.gateway.snapshots.value;
    final pairing = _currentCreated(snapshot);
    if (pairing != null) _createdPairing = pairing;
    final contactCreated = snapshot.contacts.any(
      (contact) => !_contactsBeforeCreation.contains(contact.id),
    );
    if (pairing?.typedState == PairingState.completed ||
        (_createdSessionId != null && pairing == null && contactCreated)) {
      _closeCompletedInvitation();
    }
  }

  void _closeCompletedInvitation() {
    if (_completionCloseScheduled) return;
    _completionCloseScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && Navigator.of(context).canPop()) {
        Navigator.of(context).pop();
      }
    });
    WidgetsBinding.instance.scheduleFrame();
  }

  Future<void> _focusCodeInput({bool reconnect = false}) async {
    if (!mounted) return;
    // MIUI can preserve FocusNode.hasFocus while dropping EditableText's IME
    // connection after a dialog/camera route transition. Reconnect once after
    // the route is laid out; ordinary taps only show the existing connection.
    if (reconnect) {
      _focus.unfocus();
      await Future<void>.delayed(const Duration(milliseconds: 40));
    }
    if (!mounted) return;
    FocusScope.of(context).requestFocus(_focus);
    await Future<void>.delayed(const Duration(milliseconds: 120));
    if (!mounted || !_focus.hasFocus) return;
    await SystemChannels.textInput.invokeMethod<void>('TextInput.show');
  }

  Future<void> _create() async {
    _contactsBeforeCreation = widget.gateway.snapshots.value.contacts
        .map((contact) => contact.id)
        .toSet();
    final result = await _run(
      'pairing:create',
      const CreatePairingCommandDto(),
    );
    if (result == null || !result.ok || !mounted) return;
    if (result.kind == 'pairing_queued') {
      // Keep the modal visible with an explicit waiting state. The close
      // button remains available, while the pending operation can later
      // resolve into a real pairing and QR code in this same modal.
      setState(() {
        _queued = true;
        _createdSessionId = result.resourceId;
      });
      PairingModalRegistry.instance.claim(result.resourceId ?? '');
      return;
    }
    setState(() {
      _createdSessionId = result.resourceId;
      _createdPairing = _pairingFor(result.resourceId);
      _inviteUri = result.inviteUri;
    });
    PairingModalRegistry.instance.claim(result.resourceId ?? '');
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
                    tooltip: context.strings.closeScanner,
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
      setState(() => _error = context.strings.enterSixCharacterCode);
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
              ? context.strings.invitationSavedLocally
              : context.strings.invitationJoinSent,
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
        setState(
          () => _error = result == null
              ? context.strings.invitationOperationFailed
              : BridgeErrorPresenter.localized(
                  context,
                  result!,
                  fallback: context.strings.invitationOperationFailed,
                ),
        );
      }
    });
    return result;
  }

  @override
  Widget build(BuildContext context) => AppModal(
    title: widget.mode == _PairingComposerMode.create
        ? context.strings.yourInvitation
        : context.strings.joinInvitation,
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
                  error: _error,
                  onApprove: () => _run(
                    'pairing:${pairing.id}:approve',
                    PairingAction.approve.command(pairing.id),
                  ),
                  onReject: () => _run(
                    'pairing:${pairing.id}:reject',
                    PairingAction.reject.command(pairing.id),
                  ),
                  onCancel: () => _run(
                    'pairing:${pairing.id}:cancel',
                    PairingAction.cancel.command(pairing.id),
                  ),
                  onDone: pairing.typedState == PairingState.completed
                      ? () => Navigator.of(context).pop()
                      : null,
                );
              }
              return _InvitationGenerationPlaceholder(
                busy: _operations.isActive('pairing:create'),
                queued: _queued,
                error: _error,
                onRetry: _queued || _operations.isActive('pairing:create')
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
            // Recreate EditableText's Android IME connection on every tap.
            // Some Android vendors keep FocusNode focused after a modal or
            // camera route while silently dropping the input connection.
            onFocusInput: () => _focusCodeInput(reconnect: true),
          ),
  );
}

class _InvitationGenerationPlaceholder extends StatelessWidget {
  const _InvitationGenerationPlaceholder({
    required this.busy,
    required this.queued,
    required this.error,
    required this.onRetry,
  });

  final bool busy;
  final bool queued;
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
              ? context.strings.invitationGenerating
              : queued
              ? context.strings.invitationQueued
              : context.strings.invitationWaitingForNetwork,
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 16),
        Center(child: block(width: 140, height: 140)),
        const SizedBox(height: 18),
        Center(child: block(width: 178, height: 28)),
        const SizedBox(height: 12),
        Center(child: block(width: 120, height: 18)),
        // This placeholder is rendered inside AppModal's SingleChildScrollView;
        // a flex child would receive unbounded height constraints.
        const SizedBox(height: 24),
        if (error != null) ...<Widget>[
          Text(
            error!,
            textAlign: TextAlign.center,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
          const SizedBox(height: 10),
        ],
        if (queued)
          Text(
            'Close this window to continue using the application. The invitation will appear here automatically when the connection is ready.',
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodySmall,
          )
        else
          OutlinedButton.icon(
            onPressed: onRetry,
            icon: Icon(context.torcaIcons.reconnect),
            label: Text(
              busy
                  ? context.strings.generatingInvitation
                  : context.strings.retryGeneration,
            ),
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
  late final PairingActionController _actions = PairingActionController(
    widget.gateway,
  )..addListener(_changed);
  late final Set<String> _contactsAtOpen = widget
      .gateway
      .snapshots
      .value
      .contacts
      .map((contact) => contact.id)
      .toSet();
  bool _closeScheduled = false;

  @override
  void initState() {
    super.initState();
    widget.gateway.snapshots.addListener(_snapshotChanged);
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  void _snapshotChanged() {
    if (!mounted || _closeScheduled) return;
    final snapshot = widget.gateway.snapshots.value;
    final pairing = _current(snapshot);
    final contactCreated = snapshot.contacts.any(
      (contact) => !_contactsAtOpen.contains(contact.id),
    );
    if (pairing?.typedState != PairingState.completed &&
        !(pairing == null && contactCreated)) {
      return;
    }
    _closeScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && Navigator.of(context).canPop()) {
        Navigator.of(context).pop();
      }
    });
    WidgetsBinding.instance.scheduleFrame();
  }

  @override
  void dispose() {
    widget.gateway.snapshots.removeListener(_snapshotChanged);
    _actions
      ..removeListener(_changed)
      ..dispose();
    super.dispose();
  }

  PairingDto? _current(AppSnapshotDto snapshot) {
    for (final pairing in snapshot.pairings) {
      if (pairing.id == widget.pairing.id) return pairing;
    }
    return null;
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
        height: 560,
        scrollable: true,
        child: pairing == null
            ? _TerminalPairingContent(
                completed: widget.pairing.typedState == PairingState.approved,
                onClose: () => Navigator.of(context).pop(),
              )
            : Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  _PairingSessionDetails(
                    pairing: pairing,
                    busy: _actions.busy,
                    error: _actions.error(context),
                    onApprove: () =>
                        _actions.run(PairingAction.approve, pairing.id),
                    onReject: () =>
                        _actions.run(PairingAction.reject, pairing.id),
                    onCancel: () =>
                        _actions.run(PairingAction.cancel, pairing.id),
                    onDone: pairing.typedState == PairingState.completed
                        ? () => Navigator.of(context).pop()
                        : null,
                  ),
                ],
              ),
      );
    },
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
            context.strings.joinInvitation,
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 4),
          Text(
            scanEnabled || scanBusy
                ? context.strings.enterSixCharacterCode
                : context.strings.enterSixCharacterCode,
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
              labelText: context.strings.invitationCode,
              hintText: 'ABC123',
              errorText: error,
              prefixIcon: Icon(context.torcaIcons.identity),
              suffixIcon: scanEnabled || scanBusy
                  ? IconButton(
                      tooltip: context.strings.scanQr,
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
              label: Text(
                busy
                    ? context.strings.checkingInvitation
                    : context.strings.joinInvitation,
              ),
            ),
          ),
        ],
      ),
    ),
  );
}

class _PairingSessionDetails extends StatelessWidget {
  const _PairingSessionDetails({
    required this.pairing,
    this.inviteUri,
    required this.busy,
    required this.onApprove,
    required this.onReject,
    required this.onCancel,
    this.error,
    this.onDone,
  });
  final PairingDto pairing;
  final String? inviteUri;
  final bool busy;
  final VoidCallback onApprove, onReject, onCancel;
  final String? error;
  final VoidCallback? onDone;

  bool get _canReview =>
      pairing.typedRole == PairingRole.creator &&
      (pairing.typedState == PairingState.awaitingApproval ||
          pairing.typedState == PairingState.peerJoined);

  String get _uri => inviteUri ?? pairing.inviteUri;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: <Widget>[
      if (error != null) ...<Widget>[
        Text(
          error!,
          style: TextStyle(color: Theme.of(context).colorScheme.error),
        ),
        const SizedBox(height: 12),
      ],
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
                style: context
                    .torcaCodeStyle(Theme.of(context).textTheme.bodyMedium)
                    .copyWith(letterSpacing: 1.1),
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
      if (pairing.typedState == PairingState.completed) ...<Widget>[
        Text(
          'Contact connected',
          style: Theme.of(context).textTheme.titleMedium,
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 8),
        const Text(
          'The invitation was accepted and this contact is ready to chat.',
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 18),
        FilledButton.icon(
          onPressed: onDone,
          icon: Icon(context.torcaIcons.success),
          label: Text(context.strings.done),
        ),
      ] else ...<Widget>[
        if (pairing.typedRole == PairingRole.creator &&
            pairing.typedState == PairingState.open) ...<Widget>[
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
                  label: Text(context.strings.accept),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: OutlinedButton.icon(
                  onPressed: busy ? null : onReject,
                  icon: Icon(context.torcaIcons.close),
                  label: Text(context.strings.reject),
                ),
              ),
            ],
          ),
        ] else if (pairing.typedRole == PairingRole.joiner &&
            !_canReview) ...<Widget>[
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
            label: Text(context.strings.cancelRequest),
          ),
        ] else if (pairing.typedState == PairingState.open) ...<Widget>[
          OutlinedButton.icon(
            onPressed: busy ? null : onCancel,
            style: OutlinedButton.styleFrom(
              foregroundColor: Theme.of(context).colorScheme.error,
              side: BorderSide(color: Theme.of(context).colorScheme.error),
            ),
            icon: Icon(context.torcaIcons.cancelled),
            label: Text(context.strings.cancelInvitation),
          ),
        ],
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
    final seconds = _remaining.inSeconds
        .remainder(60)
        .toString()
        .padLeft(2, '0');
    return 'Expires in $minutes:$seconds';
  }

  @override
  Widget build(BuildContext context) {
    final expired = _remaining.isNegative;
    final warning = !expired && _remaining.inSeconds <= 60;
    final scheme = Theme.of(context).colorScheme;
    final displayCode = widget.code
        .replaceAll(RegExp(r'\s+'), '')
        .toUpperCase();
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
          style: context
              .torcaCodeStyle(Theme.of(context).textTheme.headlineMedium)
              .copyWith(fontWeight: FontWeight.w700, letterSpacing: 4),
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
                        SnackBar(
                          content: Text(context.strings.invitationCodeCopied),
                        ),
                      );
                    }
                  },
            icon: Icon(context.torcaIcons.copy),
            label: Text(context.strings.copyCode),
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
        color: completed ? context.torcaColors.connectionReady : null,
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
        label: Text(
          completed ? context.strings.openConversation : context.strings.close,
        ),
      ),
    ],
  );
}
