part of 'home_screen.dart';

class _BootstrapFailureScreen extends StatelessWidget {
  const _BootstrapFailureScreen({required this.reason, this.onRetry});

  final String reason;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: Column(
        children: <Widget>[
          const Padding(
            padding: EdgeInsets.only(top: 8, right: 12),
            child: RuntimeNetworkHeader(),
          ),
          Expanded(
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 520),
                child: Padding(
                  padding: const EdgeInsets.all(28),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Icon(
                        context.torcaIcons.identity,
                        size: 64,
                        color: Theme.of(context).colorScheme.error,
                      ),
                      const SizedBox(height: 18),
                      Text(
                        context.strings.secureRuntimeNotReady,
                        style: Theme.of(context).textTheme.headlineSmall,
                        textAlign: TextAlign.center,
                      ),
                      const SizedBox(height: 12),
                      Text(
                        context.strings.runtimePreparationFailed,
                        textAlign: TextAlign.center,
                      ),
                      const SizedBox(height: 16),
                      Text(
                        reason,
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ),
                      if (onRetry != null) ...<Widget>[
                        const SizedBox(height: 22),
                        FilledButton.icon(
                          onPressed: onRetry,
                          icon: Icon(context.torcaIcons.retry),
                          label: Text(context.strings.retryNow),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    ),
  );
}

class _BootstrapProgressScreen extends StatefulWidget {
  const _BootstrapProgressScreen({required this.snapshot, this.onRetry});

  final AppSnapshotDto snapshot;
  final VoidCallback? onRetry;

  @override
  State<_BootstrapProgressScreen> createState() =>
      _BootstrapProgressScreenState();
}

class _BootstrapProgressScreenState extends State<_BootstrapProgressScreen> {
  late final DateTime _startedAt = DateTime.now();
  late final Timer _clock = Timer.periodic(const Duration(seconds: 1), (_) {
    if (mounted) setState(() {});
  });

  Duration get _elapsed => DateTime.now().difference(_startedAt);

  @override
  void dispose() {
    _clock.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme;
    const steps = <String>[
      'local_storage',
      'device_identity',
      'tor_network',
      'onion_service',
      'secure_relay',
    ];
    final projectedSteps = steps
        .map((id) => _stepFor(widget.snapshot, id))
        .toList(growable: false);
    final ready = projectedSteps
        .where((step) => step.typedState == BootstrapStepState.ready)
        .length;
    final progress =
        projectedSteps.fold<int>(0, (sum, step) => sum + step.progress) /
        (steps.length * 100);
    // The headline clock measures the complete warm-up and must not reset
    // when Tor or onion publication starts another attempt. Individual step
    // clocks below intentionally use each step's startedAtMs.
    final elapsed = _elapsed;
    final restartRequired = projectedSteps.any(
      (step) =>
          step.code == 'TOR_RESTART_REQUIRED' ||
          step.code == 'ONION_SERVICE_RESTART_REQUIRED',
    );
    return Scaffold(
      body: DecoratedBox(
        decoration: BoxDecoration(color: color.surface),
        child: SafeArea(
          child: Column(
            children: <Widget>[
              const Padding(
                padding: EdgeInsets.only(top: 8, right: 12),
                child: RuntimeNetworkHeader(),
              ),
              Expanded(
                child: SingleChildScrollView(
                  child: Center(
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 620),
                      child: Padding(
                        padding: const EdgeInsets.all(28),
                        child: Card(
                          elevation: 0,
                          color: color.surface.withValues(alpha: 0.92),
                          child: Padding(
                            padding: const EdgeInsets.all(24),
                            child: Column(
                              mainAxisSize: MainAxisSize.min,
                              children: <Widget>[
                                TorcaDeviceAvatar(
                                  label: 'Identity',
                                  identityId: widget.snapshot.identity?.id,
                                  stableDevice: true,
                                  presentation:
                                      const AvatarActivityPresentation(
                                        AvatarAnimationState.happy,
                                      ),
                                  size: 60,
                                  backgroundColor: color.primaryContainer,
                                  foregroundColor: color.onPrimaryContainer,
                                ),
                                const SizedBox(height: 16),
                                Text(
                                  'Preparing your private space',
                                  style: Theme.of(
                                    context,
                                  ).textTheme.headlineSmall,
                                  textAlign: TextAlign.center,
                                ),
                                const SizedBox(height: 8),
                                Text(
                                  'Setting up encrypted storage and a private Tor connection. You can safely leave this screen open.',
                                  style: Theme.of(context).textTheme.bodyMedium,
                                  textAlign: TextAlign.center,
                                ),
                                const SizedBox(height: 22),
                                ClipRRect(
                                  borderRadius: BorderRadius.circular(
                                    context.torcaTokens.radiusLarge,
                                  ),
                                  child: LinearProgressIndicator(
                                    value: progress.clamp(0, 1),
                                  ),
                                ),
                                const SizedBox(height: 8),
                                Text(
                                  '$ready of ${steps.length} secure checks complete  •  ${_formatDuration(elapsed)}',
                                ),
                                const SizedBox(height: 16),
                                for (final step in projectedSteps)
                                  _BootstrapStepTile(
                                    step: step,
                                    label: _bootstrapLabel(step.id),
                                    elapsed: _elapsedFor(step),
                                    retryRemaining: _retryRemaining(step),
                                  ),
                                if (widget.snapshot.typedBootstrapPhase ==
                                        BootstrapPhase.failed ||
                                    widget.snapshot.typedBootstrapPhase ==
                                        BootstrapPhase.degraded) ...<Widget>[
                                  const SizedBox(height: 12),
                                  Text(
                                    _diagnostic(widget.snapshot),
                                    textAlign: TextAlign.center,
                                    style: TextStyle(
                                      color:
                                          widget.snapshot.typedBootstrapPhase ==
                                              BootstrapPhase.degraded
                                          ? Theme.of(
                                              context,
                                            ).colorScheme.tertiary
                                          : Theme.of(context).colorScheme.error,
                                    ),
                                  ),
                                  const SizedBox(height: 12),
                                  Row(
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: <Widget>[
                                      FilledButton(
                                        onPressed: restartRequired
                                            ? null
                                            : widget.onRetry,
                                        child: Text(
                                          restartRequired
                                              ? 'Restart application'
                                              : context.strings.retryNow,
                                        ),
                                      ),
                                    ],
                                  ),
                                ],
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  BootstrapStepDto _stepFor(AppSnapshotDto snapshot, String id) {
    final match = snapshot.bootstrapSteps.where((step) => step.id == id);
    return match.isEmpty
        ? BootstrapStepDto(id: id, state: 'pending')
        : match.first;
  }

  Duration _elapsedFor(BootstrapStepDto step) {
    final startedAtMs = step.startedAtMs;
    if (startedAtMs == null) return _elapsed;
    final elapsed = DateTime.now().difference(
      DateTime.fromMillisecondsSinceEpoch(startedAtMs),
    );
    return elapsed.isNegative ? Duration.zero : elapsed;
  }

  Duration? _retryRemaining(BootstrapStepDto step) {
    final retryAtMs = step.retryAtMs;
    if (retryAtMs == null) return null;
    final remaining = DateTime.fromMillisecondsSinceEpoch(
      retryAtMs,
    ).difference(DateTime.now());
    return remaining.isNegative ? Duration.zero : remaining;
  }

  String _diagnostic(AppSnapshotDto snapshot) {
    final failed = snapshot.bootstrapSteps.where(
      (step) =>
          step.typedState == BootstrapStepState.failed ||
          step.typedState == BootstrapStepState.degraded,
    );
    final step = failed.isEmpty ? null : failed.first;
    if (step == null || step.code == null || step.code!.isEmpty) {
      return 'Secure runtime is not ready. Check diagnostics and retry.';
    }
    return '${step.id}: ${step.code}';
  }

  String _bootstrapLabel(String id) => switch (id) {
    'local_storage' => 'Local storage',
    'device_identity' => 'Device identity',
    'tor_network' => 'Tor network',
    'onion_service' => 'Onion service',
    'secure_relay' => 'Secure relay',
    _ => id,
  };

  String _formatDuration(Duration value) {
    final minutes = value.inMinutes.toString().padLeft(2, '0');
    final seconds = (value.inSeconds % 60).toString().padLeft(2, '0');
    return '$minutes:$seconds';
  }
}

class _BootstrapStepTile extends StatelessWidget {
  const _BootstrapStepTile({
    required this.step,
    required this.label,
    required this.elapsed,
    this.retryRemaining,
  });
  final BootstrapStepDto step;
  final String label;
  final Duration elapsed;
  final Duration? retryRemaining;

  @override
  Widget build(BuildContext context) {
    final ready = step.typedState == BootstrapStepState.ready;
    final running =
        step.typedState == BootstrapStepState.running ||
        step.typedState == BootstrapStepState.verifying;
    final degraded = step.typedState == BootstrapStepState.degraded;
    return Container(
      margin: const EdgeInsets.symmetric(vertical: 3),
      decoration: BoxDecoration(
        color: ready
            ? Theme.of(
                context,
              ).colorScheme.primaryContainer.withValues(alpha: 0.45)
            : null,
        borderRadius: BorderRadius.circular(context.torcaTokens.radiusLarge),
      ),
      child: ListTile(
        dense: true,
        leading: Icon(
          ready
              ? context.torcaIcons.success
              : degraded
              ? context.torcaIcons.error
              : running
              ? context.torcaIcons.reconnect
              : context.torcaIcons.queued,
          color: ready
              ? context.torcaColors.connectionReady
              : degraded
              ? Theme.of(context).colorScheme.tertiary
              : running
              ? Theme.of(context).colorScheme.primary
              : null,
        ),
        title: Text(
          step.attempt > 0 &&
                  (step.id == 'tor_network' ||
                      step.id == 'onion_service' ||
                      step.id == 'secure_relay')
              ? '$label · attempt ${step.attempt}'
              : label,
        ),
        subtitle: Text(_stateDescription(step, retryRemaining)),
        trailing: running
            ? Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (step.progress > 0) ...<Widget>[
                    Text('${step.progress}%'),
                    const SizedBox(width: 10),
                  ],
                  Text(
                    retryRemaining != null
                        ? _formatDuration(retryRemaining!)
                        : _formatDuration(elapsed),
                  ),
                  const SizedBox(width: 10),
                  const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                ],
              )
            : null,
      ),
    );
  }

  String _stateDescription(BootstrapStepDto step, Duration? retryRemaining) {
    final id = step.id;
    final value = step.typedState;
    final code = step.code;
    if (value == BootstrapStepState.running ||
        value == BootstrapStepState.verifying) {
      if (id == 'tor_network') {
        return switch (code) {
          'TOR_CONNECTING_DIRECTORY' =>
            'Opening secure channels to the Tor directory…',
          'TOR_DIRECTORY_CONSENSUS' =>
            'Channels are ready; waiting for Tor directory consensus…',
          'TOR_BOOTSTRAP_BLOCKED' =>
            'Arti reports that directory bootstrap is blocked…',
          _ => 'Preparing the embedded Tor client…',
        };
      }
      if (id == 'onion_service') {
        return switch (code) {
          'ONION_SERVICE_PUBLISHING' =>
            'Publishing this device’s private onion service…',
          _ => 'Preparing the private onion service…',
        };
      }
      return switch (id) {
        'local_storage' => 'Opening encrypted storage and checking its schema…',
        'device_identity' => 'Loading device keys and calculating fingerprint…',
        'onion_service' => 'Publishing this device’s private onion service…',
        'secure_relay' => 'Testing the embedded relay endpoint through Tor…',
        _ => 'Working securely…',
      };
    }
    return switch (value) {
      BootstrapStepState.ready => switch (id) {
        'local_storage' => 'Encrypted database is open',
        'device_identity' => 'Device identity is protected and ready',
        'tor_network' => 'Tor circuits are available',
        'onion_service' => 'Private onion service is published',
        'secure_relay' => 'Secure relay is reachable',
        _ => 'Protected and ready',
      },
      BootstrapStepState.degraded => 'Temporarily unavailable; retrying',
      BootstrapStepState.failed
          when code == 'TOR_RESTART_REQUIRED' ||
              code == 'ONION_SERVICE_RESTART_REQUIRED' =>
        'Tor did not stop safely; restart the application before retrying',
      BootstrapStepState.blocked =>
        'Waiting for the Tor network to become ready',
      BootstrapStepState.failed =>
        'Needs attention: ${code ?? 'TOR_RUNTIME_FAILED'}',
      _ => 'Waiting for the previous secure check',
    };
  }

  String _formatDuration(Duration value) {
    final minutes = value.inMinutes.toString().padLeft(2, '0');
    final seconds = (value.inSeconds % 60).toString().padLeft(2, '0');
    return '$minutes:$seconds';
  }
}
