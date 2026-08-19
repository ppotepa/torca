import 'dart:async';
import 'dart:math' as math;

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
    final transmissionActive =
        state == RadioState.receiving || state == RadioState.transmitting;
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
    final background = transmissionActive
        ? colors.errorContainer
        : prominent
        ? colors.primaryContainer
        : colors.surfaceContainerLow;
    final foreground = transmissionActive
        ? colors.onErrorContainer
        : colors.onSurface;
    return Container(
      width: double.infinity,
      color: background,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: <Widget>[
          Icon(
            state == RadioState.receiving
                ? context.torcaIcons.online
                : context.torcaIcons.radio,
            size: 18,
            color: foreground,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: foreground),
            ),
          ),
          if (session != null &&
              (state == RadioState.transmitting ||
                  state == RadioState.receiving))
            Text(
              '${(session!.burstElapsedMs / 1000).clamp(0, 10).toStringAsFixed(1)} / 10 s',
              style: Theme.of(
                context,
              ).textTheme.labelMedium?.copyWith(color: foreground),
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
    with WidgetsBindingObserver, SingleTickerProviderStateMixin {
  Timer? _burstTimer;
  int? _activePointerId;
  bool _pointerHeld = false;
  bool _transmissionActive = false;
  bool _commandBusy = false;
  bool _releaseRequested = false;
  bool _animationsEnabled = true;
  late final AnimationController _pulse;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _pulse = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 720),
    );
  }

  @override
  void didUpdateWidget(RadioPushToTalk oldWidget) {
    super.didUpdateWidget(oldWidget);
    _syncPulse();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _animationsEnabled =
        context.torcaTokens.animationDuration != Duration.zero &&
        !MediaQuery.disableAnimationsOf(context);
    _syncPulse();
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
    _pulse.dispose();
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
    final visualActive = active || _pointerHeld;
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
          child: _PttButtonVisual(
            active: visualActive,
            enabled: canPress || active,
            progress: progress,
            inputLevel: (widget.session?.inputLevelMilli ?? 0) / 1000,
            pulse: _pulse,
          ),
        ),
      ),
    );
  }

  void _setPulseActive(bool active) {
    active = active && _animationsEnabled;
    if (active && !_pulse.isAnimating) {
      _pulse.repeat();
    } else if (!active && _pulse.isAnimating) {
      _pulse.stop();
      _pulse.value = 0;
    }
  }

  void _syncPulse() {
    final state = widget.session?.typedState ?? widget.radio.typedState;
    _setPulseActive(
      _pointerHeld ||
          state == RadioState.requestingFloor ||
          state == RadioState.startingCapture ||
          state == RadioState.transmitting,
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
    _setPulseActive(true);
    final permission =
        widget.requestPermission ?? MicrophonePermission.ensureGranted;
    if (!await permission()) {
      if (mounted) {
        setState(() {
          _activePointerId = null;
          _pointerHeld = false;
        });
        _syncPulse();
        _showError(context.strings.microphonePermissionRequired);
      }
      return;
    }
    if (!mounted || !_pointerHeld || _activePointerId != pointerId) return;
    setState(() {
      _transmissionActive = true;
      _commandBusy = true;
    });
    BridgeResultDto? result;
    try {
      result = await widget.gateway.execute(
        BeginRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
      );
    } on Object {
      await _stopCaptureSafely();
      if (!mounted) return;
      setState(() {
        _activePointerId = null;
        _pointerHeld = false;
        _transmissionActive = false;
        _commandBusy = false;
      });
      _syncPulse();
      _showError(context.strings.couldNotStartRadio);
      return;
    }
    if (!mounted) {
      if (result.ok) {
        await widget.gateway.execute(
          EndRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
        );
      }
      await _stopCaptureSafely();
      return;
    }
    setState(() => _commandBusy = false);
    if (_releaseRequested || !_pointerHeld) {
      _burstTimer?.cancel();
      _transmissionActive = false;
      await widget.gateway.execute(
        EndRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
      );
      await _stopCaptureSafely();
      return;
    }
    if (!result.ok) {
      _burstTimer?.cancel();
      setState(() {
        _activePointerId = null;
        _pointerHeld = false;
        _transmissionActive = false;
      });
      await _stopCaptureSafely();
      _syncPulse();
      _showError(
        BridgeErrorPresenter.localized(
          context,
          result,
          fallback: context.strings.couldNotStartRadio,
        ),
      );
      return;
    }
    // Do not open the microphone until Rust has granted the radio floor. This
    // prevents local capture from outliving a queued/rejected transmission.
    try {
      await MicrophonePermission.setCommunicationMode(true);
      await MicrophonePermission.startNativeCapture();
    } on Object {
      await _stopCaptureSafely();
      await widget.gateway.execute(
        EndRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
      );
      if (!mounted) return;
      setState(() {
        _activePointerId = null;
        _pointerHeld = false;
        _transmissionActive = false;
      });
      _syncPulse();
      _showError(context.strings.couldNotStartRadio);
      return;
    }
    unawaited(HapticFeedback.mediumImpact());
    _burstTimer = Timer(const Duration(seconds: 10), () => unawaited(_release()));
  }

  Future<void> _release({int? pointerId, bool force = false}) async {
    if (!force && pointerId != _activePointerId) return;
    _releaseRequested = true;
    _activePointerId = null;
    _pointerHeld = false;
    if (!_transmissionActive) {
      _syncPulse();
      return;
    }
    _burstTimer?.cancel();
    if (mounted) {
      setState(() => _transmissionActive = false);
      _syncPulse();
    }
    unawaited(HapticFeedback.selectionClick());
    // Releasing PTT is a local privacy boundary: stop the microphone even
    // when the Rust command is still queued or the runtime is stalled. The
    // network-side End command remains best-effort and is sent afterwards.
    await _stopCaptureSafely();
    if (_commandBusy) return;
    await widget.gateway.execute(
      EndRadioTransmissionCommandDto(contactIdHex: widget.contact.id),
    );
  }

  Future<void> _stopCaptureSafely() async {
    try {
      await MicrophonePermission.setCommunicationMode(false);
    } catch (_) {
      // Cleanup must continue even when the platform audio route vanished.
    }
    try {
      await MicrophonePermission.stopNativeCapture();
    } catch (_) {
      // The native recorder may already have stopped during lifecycle change.
    }
  }

  void _showError(String message) {
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }
}

class _PttButtonVisual extends StatelessWidget {
  const _PttButtonVisual({
    required this.active,
    required this.enabled,
    required this.progress,
    required this.inputLevel,
    required this.pulse,
  });

  final bool active;
  final bool enabled;
  final double progress;
  final double inputLevel;
  final AnimationController pulse;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final terminal = context.torcaTokens.terminal;
    final radius = terminal
        ? BorderRadius.zero
        : BorderRadius.all(Radius.circular(context.torcaTokens.radiusLarge));
    return SizedBox.square(
      key: const ValueKey<String>('radio-ptt-button'),
      dimension: 48,
      child: AnimatedBuilder(
        animation: pulse,
        builder: (context, child) {
          final wave = math.sin(pulse.value * math.pi);
          final level = inputLevel.clamp(0.0, 1.0);
          // A quiet voice still leaves a clearly visible halo. Louder input
          // expands it far enough to remain visible below the user's finger.
          // The halo is intentionally much larger than the control. Its
          // radius must remain visible while a thumb covers the PTT button.
          // Preserve the amplitude response, but render it at 3x the original
          // radius requested for the conversation interaction.
          final haloScale = (1.18 + (level * 0.62) + (wave * 0.10)) * 3;
          return Stack(
            clipBehavior: Clip.none,
            alignment: Alignment.center,
            children: <Widget>[
              if (active)
                Transform.scale(
                  scale: haloScale,
                  child: Container(
                    key: const ValueKey<String>('radio-ptt-halo'),
                    width: 52,
                    height: 52,
                    decoration: BoxDecoration(
                      color: context.torcaColors.connectionReady.withValues(
                        alpha: 0.16 + (level * 0.12),
                      ),
                      border: Border.all(
                        color: context.torcaColors.connectionReady.withValues(
                          alpha: 0.48,
                        ),
                        width: 2,
                      ),
                      borderRadius: radius,
                    ),
                  ),
                ),
              if (active && progress > 0)
                SizedBox.square(
                  dimension: 48,
                  child: CircularProgressIndicator(
                    value: progress,
                    strokeWidth: 2.5,
                    color: colors.onError,
                    backgroundColor: colors.onError.withValues(alpha: 0.20),
                  ),
                ),
              child!,
            ],
          );
        },
        child: AnimatedContainer(
          duration: context.torcaTokens.animationDuration,
          width: 48,
          height: 48,
          decoration: BoxDecoration(
            color: colors.error.withValues(alpha: enabled ? 1 : 0.38),
            borderRadius: radius,
            border: Border.all(
              color: colors.onError.withValues(alpha: active ? 0.85 : 0.24),
            ),
            boxShadow: active
                ? <BoxShadow>[
                    BoxShadow(
                      color: colors.error.withValues(alpha: 0.35),
                      blurRadius: 12,
                    ),
                  ]
                : const <BoxShadow>[],
          ),
          child: Icon(
            active ? context.torcaIcons.pushToTalk : context.torcaIcons.radio,
            color: colors.onError.withValues(alpha: enabled ? 1 : 0.65),
            size: 24,
          ),
        ),
      ),
    );
  }
}
