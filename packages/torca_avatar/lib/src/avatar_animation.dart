import 'dart:async';

import 'package:flutter/widgets.dart';

enum AvatarPresence { online, away, offline, unknown }

enum AvatarActivity { idle, typing, speaking, listening, sending, receiving }

enum AvatarLifecycle { waking, active, sleeping }

enum AvatarAttention { none, unread, mention, incoming }

enum AvatarCondition { normal, reconnecting, error, blocked }

enum AvatarAnimationState {
  sad,
  talk,
  curious,
  bashful,
  proud,
  surprised,
  evil,
  happy,
  smirk,
  sleepy,
  confused,
}

enum AvatarVisualActivityPolicy { full, focusedOnly, staticOnly, followSystem }

/// Orthogonal, ephemeral inputs used to select an avatar presentation.
///
/// Presence is deliberately not folded into connectivity. In particular,
/// `unknown` never becomes `offline` merely because evidence is stale.
final class AvatarPresentationSignals {
  const AvatarPresentationSignals({
    this.presence = AvatarPresence.online,
    this.activity = AvatarActivity.idle,
    this.lifecycle = AvatarLifecycle.active,
    this.attention = AvatarAttention.none,
    this.condition = AvatarCondition.normal,
    this.intensity = 1,
  });

  final AvatarPresence presence;
  final AvatarActivity activity;
  final AvatarLifecycle lifecycle;
  final AvatarAttention attention;
  final AvatarCondition condition;
  final double intensity;
}

final class AvatarActivityPresentation {
  const AvatarActivityPresentation(this.state, {this.intensity = 1});

  final AvatarAnimationState state;
  final double intensity;

  /// Whether this state benefits from continuous frames. Sleeping and blocked
  /// avatars settle on a representative frame and consume no ticker time.
  bool get animates =>
      state != AvatarAnimationState.sleepy && state != AvatarAnimationState.sad;

  factory AvatarActivityPresentation.fromSignals(
    AvatarPresentationSignals signals,
  ) {
    final state = switch ((
      signals.condition,
      signals.activity,
      signals.attention,
      signals.lifecycle,
      signals.presence,
    )) {
      (AvatarCondition.blocked, _, _, _, _) => AvatarAnimationState.sad,
      (AvatarCondition.error, _, _, _, _) => AvatarAnimationState.confused,
      (_, AvatarActivity.speaking, _, _, _) => AvatarAnimationState.talk,
      (_, AvatarActivity.listening, _, _, _) => AvatarAnimationState.curious,
      (_, _, AvatarAttention.incoming || AvatarAttention.mention, _, _) =>
        AvatarAnimationState.evil,
      (_, AvatarActivity.typing, _, _, _) => AvatarAnimationState.bashful,
      (_, AvatarActivity.sending, _, _, _) => AvatarAnimationState.proud,
      (_, AvatarActivity.receiving, _, _, _) => AvatarAnimationState.surprised,
      (AvatarCondition.reconnecting, _, _, _, _) => AvatarAnimationState.happy,
      (_, _, _, AvatarLifecycle.waking, _) => AvatarAnimationState.happy,
      (_, _, AvatarAttention.unread, _, _) => AvatarAnimationState.evil,
      (_, _, _, AvatarLifecycle.sleeping, _) => AvatarAnimationState.sleepy,
      (_, _, _, _, AvatarPresence.offline || AvatarPresence.unknown) =>
        AvatarAnimationState.sleepy,
      _ => AvatarAnimationState.smirk,
    };
    return AvatarActivityPresentation(
      state,
      intensity: signals.intensity.clamp(0, 1),
    );
  }

  /// Compatibility facade for call sites that naturally expose boolean
  /// runtime projections. New features should prefer [fromSignals].
  static AvatarActivityPresentation resolve({
    bool blocked = false,
    bool talking = false,
    bool listening = false,
    bool typing = false,
    bool sending = false,
    bool receiving = false,
    bool attention = false,
    bool waking = false,
    bool online = true,
    bool error = false,
    double intensity = 1,
  }) => AvatarActivityPresentation.fromSignals(
    AvatarPresentationSignals(
      presence: online ? AvatarPresence.online : AvatarPresence.offline,
      activity: talking
          ? AvatarActivity.speaking
          : listening
          ? AvatarActivity.listening
          : typing
          ? AvatarActivity.typing
          : sending
          ? AvatarActivity.sending
          : receiving
          ? AvatarActivity.receiving
          : AvatarActivity.idle,
      lifecycle: waking ? AvatarLifecycle.waking : AvatarLifecycle.active,
      attention: attention ? AvatarAttention.unread : AvatarAttention.none,
      condition: blocked
          ? AvatarCondition.blocked
          : error
          ? AvatarCondition.error
          : AvatarCondition.normal,
      intensity: intensity,
    ),
  );
}

extension AvatarAnimationSpec on AvatarAnimationState {
  int get frameCount => switch (this) {
    AvatarAnimationState.sleepy => 4,
    AvatarAnimationState.sad => 1,
    AvatarAnimationState.happy ||
    AvatarAnimationState.evil ||
    AvatarAnimationState.confused => 6,
    AvatarAnimationState.talk => 8,
    _ => 6,
  };

  Duration get frameDuration => switch (this) {
    AvatarAnimationState.sleepy => const Duration(milliseconds: 420),
    AvatarAnimationState.smirk => const Duration(milliseconds: 180),
    AvatarAnimationState.sad => const Duration(days: 1),
    AvatarAnimationState.talk ||
    AvatarAnimationState.bashful => const Duration(milliseconds: 100),
    AvatarAnimationState.curious => const Duration(milliseconds: 140),
    _ => const Duration(milliseconds: 160),
  };

  /// A one-to-one mapping to values accepted by avatar_genome's
  /// `v4.faceAnimation` catalog. Keeping this injective makes runtime states
  /// visually distinguishable and prevents unsupported names from silently
  /// becoming a static face.
  String get generatorAnimation => name;

  Map<String, Object> get generatorOverrides => <String, Object>{
    'v4.faceAnimation': generatorAnimation,
    'v4.mouthMotionStyle': switch (this) {
      AvatarAnimationState.talk => 'talkNormal',
      AvatarAnimationState.sleepy => 'breathLoop',
      _ => 'none',
    },
  };
}

/// One low-frequency clock shared by all currently animating, visible avatars.
final class AvatarFrameClock extends ChangeNotifier
    with WidgetsBindingObserver {
  AvatarFrameClock._();

  static final AvatarFrameClock instance = AvatarFrameClock._();
  Timer? _timer;
  int _clients = 0;
  int _focusedClients = 0;
  bool _foreground = true;
  AvatarVisualActivityPolicy _policy = AvatarVisualActivityPolicy.followSystem;

  int get elapsedMilliseconds => DateTime.now().millisecondsSinceEpoch;

  AvatarVisualActivityPolicy get policy => _policy;

  void setPolicy(AvatarVisualActivityPolicy policy) {
    if (_policy == policy) return;
    _policy = policy;
    if (policy == AvatarVisualActivityPolicy.staticOnly ||
        (policy == AvatarVisualActivityPolicy.focusedOnly &&
            _focusedClients == 0)) {
      _timer?.cancel();
      _timer = null;
    } else {
      _start();
    }
    notifyListeners();
  }

  bool allowsAnimation({required bool focused}) => switch (_policy) {
    AvatarVisualActivityPolicy.staticOnly => false,
    AvatarVisualActivityPolicy.focusedOnly => focused,
    AvatarVisualActivityPolicy.full ||
    AvatarVisualActivityPolicy.followSystem => true,
  };

  @visibleForTesting
  int get clients => _clients;

  void attach({bool focused = false}) {
    _clients += 1;
    if (focused) _focusedClients += 1;
    if (_clients == 1) {
      WidgetsBinding.instance.addObserver(this);
      _start();
    } else if (focused) {
      _restartForCadence();
    }
  }

  void detach({bool focused = false}) {
    if (focused) _focusedClients = (_focusedClients - 1).clamp(0, 1 << 30);
    _clients = (_clients - 1).clamp(0, 1 << 30);
    if (_clients == 0 ||
        (_policy == AvatarVisualActivityPolicy.focusedOnly &&
            _focusedClients == 0)) {
      _timer?.cancel();
      _timer = null;
      if (_clients == 0) WidgetsBinding.instance.removeObserver(this);
    } else if (focused) {
      _restartForCadence();
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    _foreground = state == AppLifecycleState.resumed;
    if (_foreground) {
      _start();
    } else {
      _timer?.cancel();
      _timer = null;
    }
  }

  void _start() {
    if (!_foreground ||
        _clients == 0 ||
        _timer != null ||
        _policy == AvatarVisualActivityPolicy.staticOnly ||
        (_policy == AvatarVisualActivityPolicy.focusedOnly &&
            _focusedClients == 0))
      return;
    // Focused/speaking avatars retain the responsive 10 fps cadence. Large
    // contact/chat lists share a 4 fps clock, which avoids rebuilding every
    // visible avatar ten times per second while preserving animation.
    final cadence = _focusedClients > 0
        ? const Duration(milliseconds: 100)
        : const Duration(milliseconds: 250);
    _timer = Timer.periodic(cadence, (_) {
      notifyListeners();
    });
  }

  void _restartForCadence() {
    if (_timer == null) return;
    _timer?.cancel();
    _timer = null;
    _start();
  }
}
