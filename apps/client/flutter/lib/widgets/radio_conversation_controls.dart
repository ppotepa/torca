import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
import '../platform/microphone_permission.dart';
import 'bridge_error_presenter.dart';

class RadioConversationStatus extends StatelessWidget {
  const RadioConversationStatus({
    required this.contact,
    required this.radio,
    required this.session,
    required this.timeline,
    super.key,
  });

  final ContactDto contact;
  final RadioContactDto? radio;
  final RadioSessionDto? session;
  final List<RadioTimelineEventDto> timeline;

  @override
  Widget build(BuildContext context) => Column(
    mainAxisSize: MainAxisSize.min,
    children: <Widget>[
      if (radio?.localEnabled == true) _status(context, radio!),
      if (timeline.isNotEmpty) _notice(context, timeline.last),
    ],
  );

  Widget _status(BuildContext context, RadioContactDto radio) {
    final state = session?.typedState ?? radio.typedState;
    final prominent =
        state == RadioState.receiving ||
        state == RadioState.transmitting ||
        state == RadioState.requestingFloor ||
        state == RadioState.startingCapture;
    final colors = Theme.of(context).colorScheme;
    final text = switch (state) {
      RadioState.available ||
      RadioState.waitingForPeer => context.strings.radioWaitingForPeer,
      RadioState.connecting => context.strings.radioConnecting,
      RadioState.ready => context.strings.radioReady,
      RadioState.requestingFloor => context.strings.radioRequestingFloor,
      RadioState.startingCapture => context.strings.radioRequestingFloor,
      RadioState.transmitting => context.strings.radioTransmitting,
      RadioState.receiving => context.strings.radioReceiving(
        contact.displayName,
      ),
      RadioState.reconnecting => context.strings.radioReconnecting,
      _ => context.strings.radioUnavailable,
    };
    return Container(
      width: double.infinity,
      color: prominent ? colors.primaryContainer : colors.surfaceContainerLow,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: <Widget>[
          Icon(
            state == RadioState.receiving
                ? context.torcaIcons.online
                : context.torcaIcons.radio,
            size: 18,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(text, maxLines: 1, overflow: TextOverflow.ellipsis),
          ),
          if (session != null &&
              (state == RadioState.transmitting ||
                  state == RadioState.receiving))
            Text(
              '${(session!.burstElapsedMs / 1000).clamp(0, 10).toStringAsFixed(1)} / 10 s',
              style: Theme.of(context).textTheme.labelMedium,
            ),
        ],
      ),
    );
  }

  Widget _notice(BuildContext context, RadioTimelineEventDto event) {
    final actor = switch (event.actor) {
      'local' => context.strings.senderYou,
      'remote' => contact.displayName,
      _ => context.strings.radioMode,
    };
    final label = switch (event.kind) {
      'enabled' => context.strings.radioEnabledBy(actor),
      'disabled' => context.strings.radioDisabledBy(actor),
      'ready' => context.strings.radioChannelReady,
      'interrupted' => context.strings.radioChannelInterrupted,
      'restored' => context.strings.radioChannelRestored,
      _ => context.strings.radioMode,
    };
    final occurred = DateTime.fromMillisecondsSinceEpoch(
      event.occurredAtMs,
    ).toLocal();
    final time =
        '${occurred.hour.toString().padLeft(2, '0')}:${occurred.minute.toString().padLeft(2, '0')}';
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 6, 16, 2),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: <Widget>[
          Icon(context.torcaIcons.radio, size: 14),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              '$label · $time',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.labelSmall,
            ),
          ),
        ],
      ),
    );
  }
}

/// Hold-to-talk interaction owner. Pointer state, permission, lifecycle and
/// backend commands stay together so a permission dialog, route disposal or
/// background transition can never leave capture active.
class RadioPushToTalk extends StatefulWidget {
  const RadioPushToTalk({
    required this.gateway,
    required this.contact,
    required this.radio,
    required this.session,
    this.disabled = false,
    this.requestPermission,
    super.key,
  });

  final EngineGateway gateway;
  final ContactDto contact;
  final RadioContactDto radio;
  final RadioSessionDto? session;
  final bool disabled;
  final Future<bool> Function()? requestPermission;

  @override
  State<RadioPushToTalk> createState() => _RadioPushToTalkState();
}

class _RadioPushToTalkState extends State<RadioPushToTalk>
    with WidgetsBindingObserver {
  Timer? _burstTimer;
  int? _activePointerId;
  bool _pointerHeld = false;
  bool _transmissionActive = false;
  bool _commandBusy = false;
  bool _releaseRequested = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // `inactive` is also emitted for transient Android focus changes (for
    // example a system permission surface). It must not terminate a held PTT
    // burst. A real pause, detach or disposal still releases the microphone.
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.detached) {
      unawaited(_release(force: true));
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _burstTimer?.cancel();
    unawaited(_release(force: true));
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = widget.session?.typedState ?? widget.radio.typedState;
    final active =
        state == RadioState.requestingFloor ||
        state == RadioState.startingCapture ||
        state == RadioState.transmitting;
    final canPress =
        !_commandBusy &&
        !widget.disabled &&
        widget.contact.typedStatus != ContactStatus.blocked &&
        state == RadioState.ready;
    final elapsed = widget.session?.burstElapsedMs ?? 0;
    final maximum = widget.session?.maxBurstMs ?? 10000;
    final progress = active && maximum > 0
        ? (elapsed / maximum).clamp(0.0, 1.0)
        : 0.0;
    return Semantics(
      button: true,
      enabled: canPress,
      label: context.strings.radioReady,
      // A tap recognizer is deliberately not used here. A PTT press owns one
      // raw pointer until its matching up/cancel event. Otherwise a tiny drag
      // can lose Flutter's gesture arena and release capture while the user is
      // still holding the button.
      child: Listener(
        behavior: HitTestBehavior.opaque,
        onPointerDown: canPress
            ? (event) => unawaited(_press(event.pointer))
            : null,
        onPointerUp: _pointerHeld || _transmissionActive
            ? (event) => unawaited(_release(pointerId: event.pointer))
            : null,
        onPointerCancel: _pointerHeld || _transmissionActive
            ? (event) => unawaited(_release(pointerId: event.pointer))
            : null,
        child: Tooltip(
          message: _tooltip(context, state),
          child: SizedBox.square(
            dimension: 48,
            child: Stack(
              alignment: Alignment.center,
              children: <Widget>[
                if (active)
                  SizedBox.square(
                    dimension: 46,
                    child: CircularProgressIndicator(
                      value: progress,
                      strokeWidth: 3,
                    ),
                  ),
                Icon(
                  context.torcaIcons.pushToTalk,
                  color: canPress || active
                      ? Theme.of(context).colorScheme.primary
                      : Theme.of(context).disabledColor,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  String _tooltip(BuildContext context, RadioState state) => switch (state) {
    RadioState.ready => context.strings.radioReady,
    RadioState.receiving => context.strings.radioReceiving(
      widget.contact.displayName,
    ),
    RadioState.waitingForPeer ||
    RadioState.available => context.strings.radioWaitingForPeer,
    RadioState.startingCapture => context.strings.radioRequestingFloor,
    _ => context.strings.radioUnavailable,
  };

  Future<void> _press(int pointerId) async {
    if (_pointerHeld || _transmissionActive || _commandBusy) return;
    setState(() {
      _activePointerId = pointerId;
      _pointerHeld = true;
      _releaseRequested = false;
    });
    final permission =
        widget.requestPermission ?? MicrophonePermission.ensureGranted;
    if (!await permission()) {
      if (mounted) {
        setState(() {
          _activePointerId = null;
          _pointerHeld = false;
        });
        _showError(context.strings.microphonePermissionRequired);
      }
      return;
    }
    if (!mounted || !_pointerHeld || _activePointerId != pointerId) return;
    setState(() {
      _transmissionActive = true;
      _commandBusy = true;
    });
    unawaited(HapticFeedback.mediumImpact());
    _burstTimer = Timer(
      const Duration(seconds: 10),
      () => unawaited(_release()),
    );
    final result = await widget.gateway.execute(
      BeginRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
    );
    if (!mounted) {
      if (result.ok) {
        await widget.gateway.execute(
          EndRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
        );
      }
      return;
    }
    setState(() => _commandBusy = false);
    if (_releaseRequested || !_pointerHeld) {
      _burstTimer?.cancel();
      _transmissionActive = false;
      await widget.gateway.execute(
        EndRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
      );
      return;
    }
    if (!result.ok) {
      _burstTimer?.cancel();
      setState(() {
        _activePointerId = null;
        _pointerHeld = false;
        _transmissionActive = false;
      });
      _showError(
        BridgeErrorPresenter.localized(
          context,
          result,
          fallback: context.strings.couldNotStartRadio,
        ),
      );
    }
  }

  Future<void> _release({int? pointerId, bool force = false}) async {
    if (!force && pointerId != _activePointerId) return;
    _releaseRequested = true;
    _activePointerId = null;
    _pointerHeld = false;
    if (!_transmissionActive) return;
    _burstTimer?.cancel();
    if (mounted) setState(() => _transmissionActive = false);
    unawaited(HapticFeedback.selectionClick());
    if (_commandBusy) return;
    await widget.gateway.execute(
      EndRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
    );
  }

  void _showError(String message) {
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }
}
