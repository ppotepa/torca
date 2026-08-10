import 'package:flutter/material.dart';

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

/// Compact Tor and relay state.  It breathes while verifying and flashes only
/// after a redacted runtime transport activity observation.
class RuntimeNetworkStatus extends StatelessWidget {
  const RuntimeNetworkStatus({required this.snapshot, super.key});

  final AppSnapshotDto snapshot;

  @override
  Widget build(BuildContext context) {
    final wide = MediaQuery.sizeOf(context).width >= 700;
    return Semantics(
      label:
          'Network status: Tor ${snapshot.transport.tor.state}, relay ${snapshot.transport.relay.state}',
      child: Padding(
        padding: const EdgeInsets.only(right: 4),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            _TransportLight(
              key: const ValueKey<String>('tor-status-light'),
              label: 'Tor',
              icon: Icons.shield_outlined,
              indicator: snapshot.transport.tor,
              showLabel: wide,
            ),
            const SizedBox(width: 4),
            _TransportLight(
              key: const ValueKey<String>('relay-status-light'),
              label: 'Relay',
              icon: Icons.hub_outlined,
              indicator: snapshot.transport.relay,
              showLabel: wide,
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
    super.key,
  });

  final String label;
  final IconData icon;
  final TransportIndicatorDto indicator;
  final bool showLabel;

  @override
  State<_TransportLight> createState() => _TransportLightState();
}

class _TransportLightState extends State<_TransportLight>
    with TickerProviderStateMixin {
  late final AnimationController _breathing = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1200),
  );
  late final AnimationController _pulse = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 720),
  );

  @override
  void initState() {
    super.initState();
    _syncBreathing();
  }

  @override
  void didUpdateWidget(covariant _TransportLight oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.indicator.activitySequence !=
            widget.indicator.activitySequence &&
        widget.indicator.activitySequence > 0) {
      _pulse.forward(from: 0);
    }
    _syncBreathing();
  }

  void _syncBreathing() {
    if (_isVerifying(widget.indicator.state)) {
      if (!_breathing.isAnimating) _breathing.repeat(reverse: true);
    } else {
      _breathing.stop();
      _breathing.value = 0;
    }
  }

  @override
  void dispose() {
    _breathing.dispose();
    _pulse.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = _color(context, widget.indicator.state);
    final status = _statusLabel(widget.indicator.state);
    final latency = widget.indicator.latencyMs == null
        ? ''
        : ' · ${widget.indicator.latencyMs} ms';
    return Tooltip(
      message: '${widget.label}: $status$latency',
      child: AnimatedBuilder(
        animation: Listenable.merge(<Listenable>[_breathing, _pulse]),
        builder: (context, child) {
          final glow =
              (_breathing.value * 0.35) +
              (_pulse.value < 0.45
                  ? _pulse.value * 1.4
                  : (1 - _pulse.value) * 1.1);
          return Container(
            padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 6),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(18),
              color: color.withValues(alpha: 0.10 + glow.clamp(0, 0.35)),
              border: Border.all(
                color: color.withValues(alpha: 0.38 + glow.clamp(0, 0.45)),
              ),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Stack(
                  alignment: Alignment.center,
                  children: <Widget>[
                    Icon(widget.icon, size: 17, color: color),
                    Positioned(
                      right: -1,
                      bottom: -1,
                      child: Container(
                        width: 7 + (glow * 4),
                        height: 7 + (glow * 4),
                        decoration: BoxDecoration(
                          color: color,
                          shape: BoxShape.circle,
                        ),
                      ),
                    ),
                  ],
                ),
                if (widget.showLabel) ...<Widget>[
                  const SizedBox(width: 5),
                  Text(
                    widget.label,
                    style: Theme.of(context).textTheme.labelMedium,
                  ),
                ],
              ],
            ),
          );
        },
      ),
    );
  }
}

bool _isVerifying(String state) =>
    state == 'starting' || state == 'checking' || state == 'connecting';

String _statusLabel(String state) => switch (state) {
  'ready' || 'healthy' => 'connected',
  'starting' || 'checking' || 'connecting' => 'verifying connection',
  'degraded' => 'degraded',
  'failed' || 'unreachable' => 'unavailable',
  _ => 'offline',
};

Color _color(BuildContext context, String state) => switch (state) {
  'ready' || 'healthy' => context.semanticColors.connectionReady,
  'starting' ||
  'checking' ||
  'connecting' => context.semanticColors.connectionConnecting,
  'degraded' => context.semanticColors.warning,
  'failed' || 'unreachable' => context.semanticColors.connectionOffline,
  _ => context.semanticColors.connectionOffline,
};
