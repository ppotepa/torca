import 'dart:async';

import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../theme/app_semantic_colors.dart';

/// Makes process-runtime state available to every route without duplicating
/// gateway wiring in individual screens.
class RuntimeStatusScope extends InheritedWidget {
  const RuntimeStatusScope({
    required this.gateway,
    required super.child,
    super.key,
  });

  final EngineGateway gateway;

  static RuntimeStatusScope of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<RuntimeStatusScope>()!;

  static RuntimeStatusScope? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<RuntimeStatusScope>();

  @override
  bool updateShouldNotify(RuntimeStatusScope oldWidget) =>
      gateway != oldWidget.gateway;
}

/// Reusable top bar used by every normal route.
class RuntimeAppBar extends StatelessWidget implements PreferredSizeWidget {
  const RuntimeAppBar({
    required this.title,
    this.actions = const <Widget>[],
    this.automaticallyImplyLeading = true,
    this.titleSpacing,
    super.key,
  });

  final Widget title;
  final List<Widget> actions;
  final bool automaticallyImplyLeading;
  final double? titleSpacing;

  @override
  Size get preferredSize => const Size.fromHeight(kToolbarHeight);

  @override
  Widget build(BuildContext context) {
    final scope = RuntimeStatusScope.maybeOf(context);
    if (scope == null) {
      return AppBar(
        title: title,
        titleSpacing: titleSpacing,
        automaticallyImplyLeading: automaticallyImplyLeading,
        actions: actions,
      );
    }
    final gateway = scope.gateway;
    return ValueListenableBuilder<AppSnapshotDto>(
      valueListenable: gateway.snapshots,
      builder: (context, snapshot, _) => AppBar(
        title: title,
        titleSpacing: titleSpacing,
        automaticallyImplyLeading: automaticallyImplyLeading,
        actions: <Widget>[
          RuntimeNetworkStatus(snapshot: snapshot),
          ...actions,
        ],
      ),
    );
  }
}

/// Process-wide Tor, relay and P2P monitor. Payloads never enter this widget;
/// only monotonic TX/RX counters and health projections cross the ABI.
class RuntimeNetworkStatus extends StatefulWidget {
  const RuntimeNetworkStatus({required this.snapshot, super.key});

  final AppSnapshotDto snapshot;

  @override
  State<RuntimeNetworkStatus> createState() => _RuntimeNetworkStatusState();
}

class _RuntimeNetworkStatusState extends State<RuntimeNetworkStatus> {
  static const _staleAfterTicks = 3;
  var _ticksSinceObservation = 0;
  Timer? _freshnessTimer;

  @override
  void initState() {
    super.initState();
    _freshnessTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) setState(() => _ticksSinceObservation += 1);
    });
  }

  @override
  void didUpdateWidget(covariant RuntimeNetworkStatus oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.snapshot, widget.snapshot)) {
      _ticksSinceObservation = 0;
    }
  }

  @override
  void dispose() {
    _freshnessTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final wide = MediaQuery.sizeOf(context).width >= 700;
    final stale = _ticksSinceObservation >= _staleAfterTicks;
    return Semantics(
      label:
          'Network status: Tor ${widget.snapshot.transport.tor.state}, relay ${widget.snapshot.transport.relay.state}, P2P ${widget.snapshot.transport.peer.state}${stale ? ', monitoring stale' : ''}',
      child: Padding(
        padding: const EdgeInsets.only(right: 4),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            if (widget.snapshot.pendingOperations.isNotEmpty) ...<Widget>[
              Tooltip(
                message:
                    '${widget.snapshot.pendingOperations.length} operation(s) waiting for connectivity',
                child: Badge(
                  label: Text('${widget.snapshot.pendingOperations.length}'),
                  child: Icon(
                    context.torcaIcons.reconnect,
                    size: 18,
                    color: Theme.of(context).colorScheme.tertiary,
                  ),
                ),
              ),
              const SizedBox(width: 7),
            ],
            _TransportLight(
              key: const ValueKey<String>('tor-status-light'),
              label: 'Tor',
              icon: context.torcaIcons.identity,
              indicator: widget.snapshot.transport.tor,
              showLabel: wide,
              stale: stale,
            ),
            const SizedBox(width: 4),
            _TransportLight(
              key: const ValueKey<String>('peer-status-light'),
              label: 'P2P',
              icon: context.torcaIcons.online,
              indicator: widget.snapshot.transport.peer,
              showLabel: wide,
              stale: stale,
            ),
            const SizedBox(width: 4),
            _TransportLight(
              key: const ValueKey<String>('relay-status-light'),
              label: 'Relay',
              icon: context.torcaIcons.link,
              indicator: widget.snapshot.transport.relay,
              showLabel: wide,
              stale: stale,
            ),
          ],
        ),
      ),
    );
  }
}

/// Header-only variant for bootstrap and failure surfaces that do not use a
/// normal Scaffold app bar.
class RuntimeNetworkHeader extends StatelessWidget {
  const RuntimeNetworkHeader({super.key});

  @override
  Widget build(BuildContext context) {
    final scope = RuntimeStatusScope.maybeOf(context);
    if (scope == null) return const SizedBox.shrink();
    final gateway = scope.gateway;
    return Align(
      alignment: Alignment.centerRight,
      child: ValueListenableBuilder<AppSnapshotDto>(
        valueListenable: gateway.snapshots,
        builder: (context, snapshot, _) =>
            RuntimeNetworkStatus(snapshot: snapshot),
      ),
    );
  }
}

class _TransportLight extends StatefulWidget {
  const _TransportLight({
    required this.label,
    required this.icon,
    required this.indicator,
    required this.showLabel,
    required this.stale,
    super.key,
  });

  final String label;
  final IconData icon;
  final TransportIndicatorDto indicator;
  final bool showLabel;
  final bool stale;

  @override
  State<_TransportLight> createState() => _TransportLightState();
}

class _TransportLightState extends State<_TransportLight>
    with TickerProviderStateMixin {
  late final AnimationController _breathing = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1200),
  );
  late final AnimationController _txPulse = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 240),
  );
  late final AnimationController _rxPulse = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 240),
  );
  bool _animationsEnabled = true;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _animationsEnabled =
        context.torcaTokens.animationDuration != Duration.zero &&
        !MediaQuery.disableAnimationsOf(context);
    _syncBreathing();
    if (!_animationsEnabled) {
      _txPulse
        ..stop()
        ..value = 0;
      _rxPulse
        ..stop()
        ..value = 0;
    }
  }

  @override
  void didUpdateWidget(covariant _TransportLight oldWidget) {
    super.didUpdateWidget(oldWidget);
    final txChanged =
        oldWidget.indicator.txSequence != widget.indicator.txSequence &&
        widget.indicator.txSequence > 0;
    final rxChanged =
        oldWidget.indicator.rxSequence != widget.indicator.rxSequence &&
        widget.indicator.rxSequence > 0;
    if (_animationsEnabled && txChanged) {
      _txPulse.forward(from: 0);
    }
    if (_animationsEnabled && rxChanged) {
      // RX is an independent direction. Do not delay it behind TX: a single
      // snapshot may legitimately contain both directions, and the LEDs must
      // reflect the observed counters rather than an inferred request order.
      _rxPulse.forward(from: 0);
    }
    _syncBreathing();
    if (_animationsEnabled &&
        oldWidget.indicator.typedState != widget.indicator.typedState &&
        _isAlarmState(widget.indicator.typedState)) {
      _breathing.forward(from: 0);
    }
  }

  void _syncBreathing() {
    if (_animationsEnabled && _isPulsingLink(widget.indicator.typedState)) {
      if (!_breathing.isAnimating) _breathing.repeat(reverse: true);
    } else {
      _breathing.stop();
      _breathing.value = 0;
    }
  }

  @override
  void dispose() {
    _breathing.dispose();
    _txPulse.dispose();
    _rxPulse.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final stateColor = widget.stale
        ? Theme.of(context).colorScheme.outline
        : _color(context, widget.indicator.typedState);
    final iconColor = widget.stale
        ? Theme.of(context).colorScheme.outline
        : Theme.of(context).colorScheme.onSurface;
    final status = widget.stale
        ? 'monitoring stale'
        : _statusLabel(widget.indicator.typedState);
    final latency = widget.indicator.latencyMs == null
        ? ''
        : ' · ${widget.indicator.latencyMs} ms';
    final pressure =
        widget.indicator.inFlight == 0 && widget.indicator.queued == 0
        ? ''
        : ' · ${widget.indicator.inFlight} active, ${widget.indicator.queued} queued';
    return Tooltip(
      message:
          '${widget.label}: $status$latency$pressure (${widget.indicator.code})',
      child: RepaintBoundary(
        child: AnimatedBuilder(
          animation: Listenable.merge(<Listenable>[
            _breathing,
            _txPulse,
            _rxPulse,
          ]),
          builder: (context, child) {
            final connecting =
                !widget.stale && _isPulsingLink(widget.indicator.typedState);
            final failed =
                !widget.stale && _isAlarmState(widget.indicator.typedState);
            final linkActive =
                !widget.stale && _linkActive(widget.indicator.typedState);
            return Padding(
              padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 6),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Icon(widget.icon, size: 17, color: iconColor),
                  if (widget.showLabel) ...<Widget>[
                    const SizedBox(width: 5),
                    Text(
                      widget.label,
                      style: Theme.of(context).textTheme.labelMedium,
                    ),
                  ],
                  const SizedBox(width: 5),
                  _EthernetLed(
                    key: ValueKey<String>('${widget.label}-link-led'),
                    label: 'LINK',
                    active:
                        linkActive ||
                        failed ||
                        (connecting && _breathing.value > 0.52),
                    color: stateColor,
                  ),
                  const SizedBox(width: 3),
                  _EthernetLed(
                    key: ValueKey<String>('${widget.label}-tx-led'),
                    label: 'TX',
                    active: _txPulse.isAnimating,
                    color: context.semanticColors.activityTransmit,
                  ),
                  const SizedBox(width: 3),
                  _EthernetLed(
                    key: ValueKey<String>('${widget.label}-rx-led'),
                    label: 'RX',
                    active: _rxPulse.isAnimating,
                    color: context.semanticColors.activityReceive,
                  ),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}

class _EthernetLed extends StatelessWidget {
  const _EthernetLed({
    required this.label,
    required this.active,
    required this.color,
    super.key,
  });

  final String label;
  final bool active;
  final Color color;

  @override
  Widget build(BuildContext context) => Semantics(
    label: '$label ${active ? 'activity' : 'idle'}',
    child: Container(
      width: 5,
      height: 9,
      decoration: BoxDecoration(
                    color: active ? color : context.semanticColors.activityIdle,
        borderRadius: BorderRadius.circular(
          context.torcaTokens.terminal ? 0 : 1,
        ),
        boxShadow: active
            ? <BoxShadow>[
                BoxShadow(color: color.withValues(alpha: 0.72), blurRadius: 5),
              ]
            : null,
      ),
    ),
  );
}

bool _isVerifying(TransportState state) => const {
  TransportState.starting,
  TransportState.checking,
  TransportState.connecting,
}.contains(state);

bool _isPulsingLink(TransportState state) => _isVerifying(state);

bool _isAlarmState(TransportState state) => const {
  TransportState.degraded,
  TransportState.failed,
  TransportState.unreachable,
  TransportState.disconnected,
}.contains(state);

bool _linkActive(TransportState state) =>
    state == TransportState.ready || state == TransportState.healthy;

String _statusLabel(TransportState state) => switch (state) {
  TransportState.ready || TransportState.healthy => 'connected',
  TransportState.starting ||
  TransportState.checking ||
  TransportState.connecting => 'connecting',
  TransportState.degraded => 'degraded',
  TransportState.failed || TransportState.unreachable => 'unavailable',
  TransportState.inactive => 'inactive',
  TransportState.disconnected => 'disconnected',
  _ => 'offline',
};

Color _color(BuildContext context, TransportState state) => switch (state) {
  TransportState.ready ||
  TransportState.healthy => context.semanticColors.connectionReady,
  TransportState.starting ||
  TransportState.checking ||
  TransportState.connecting => context.semanticColors.connectionConnecting,
  TransportState.degraded => context.semanticColors.warning,
  TransportState.failed ||
  TransportState.unreachable => context.semanticColors.connectionOffline,
  TransportState.inactive ||
  TransportState.stopped => context.semanticColors.inactiveIndicator,
  TransportState.disconnected => context.semanticColors.connectionOffline,
  _ => context.semanticColors.connectionConnecting,
};
