import 'dart:async';
import 'dart:io';

import 'package:audioplayers/audioplayers.dart';
import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';
import 'attachment_tile.dart';

/// One playback lane for the whole client. A conversation can contain many
/// voice messages, but only one native player should own an audio focus route.
final class VoicePlaybackController extends ChangeNotifier {
  VoicePlaybackController._() {
    _player.onPositionChanged.listen((value) {
      _position = value;
      notifyListeners();
    });
    _player.onDurationChanged.listen((value) {
      _duration = value;
      notifyListeners();
    });
    _player.onPlayerStateChanged.listen((state) {
      _playing = state == PlayerState.playing;
      if (state == PlayerState.completed) {
        _playedIds.add(_activeId ?? '');
        unawaited(_finishCurrent());
      }
      notifyListeners();
    });
  }

  static final VoicePlaybackController instance = VoicePlaybackController._();

  final AudioPlayer _player = AudioPlayer();
  final Set<String> _playedIds = <String>{};
  String? _activeId;
  String? _activePath;
  Duration _position = Duration.zero;
  Duration _duration = Duration.zero;
  bool _playing = false;

  String? get activeId => _activeId;
  Duration get position => _position;
  Duration get duration => _duration;
  bool get playing => _playing;

  bool wasPlayed(String id) => _playedIds.contains(id);

  Future<void> toggle(String id, String path) async {
    if (_activeId == id) {
      if (_playing) {
        await _player.pause();
      } else {
        await _player.resume();
      }
      return;
    }
    await _stopCurrent();
    _activeId = id;
    _activePath = path;
    _position = Duration.zero;
    _duration = Duration.zero;
    notifyListeners();
    await _player.play(DeviceFileSource(path));
  }

  Future<void> stop() => _stopCurrent();

  Future<void> _stopCurrent() async {
    await _player.stop();
    await _deleteActivePath();
    _activeId = null;
    _activePath = null;
    _position = Duration.zero;
    _duration = Duration.zero;
    _playing = false;
    notifyListeners();
  }

  Future<void> _finishCurrent() async {
    await _deleteActivePath();
    _activeId = null;
    _activePath = null;
    _position = Duration.zero;
    _duration = Duration.zero;
    _playing = false;
    notifyListeners();
  }

  Future<void> _deleteActivePath() async {
    final path = _activePath;
    if (path == null) return;
    final file = File(path);
    if (await file.exists()) await file.delete();
  }

  @override
  void dispose() {
    unawaited(_player.dispose());
    super.dispose();
  }
}

class VoiceMessageTile extends StatefulWidget {
  const VoiceMessageTile({
    required this.attachment,
    required this.materialize,
    required this.onRetry,
    required this.onCancel,
    this.onPlayed,
    this.operationBusy = false,
    super.key,
  });

  final AttachmentDto attachment;
  final Future<String?> Function() materialize;
  final VoidCallback onRetry;
  final VoidCallback onCancel;
  final VoidCallback? onPlayed;
  final bool operationBusy;

  @override
  State<VoiceMessageTile> createState() => _VoiceMessageTileState();
}

class _VoiceMessageTileState extends State<VoiceMessageTile> {
  VoicePlaybackController get _playback => VoicePlaybackController.instance;
  bool _loading = false;
  bool _notifiedPlayed = false;

  @override
  void initState() {
    super.initState();
    _playback.addListener(_changed);
  }

  @override
  void dispose() {
    _playback.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    if (!mounted) return;
    final played = _playback.wasPlayed(widget.attachment.id);
    if (played && !_notifiedPlayed) {
      _notifiedPlayed = true;
      widget.onPlayed?.call();
    }
    setState(() {});
  }

  Future<void> _toggle() async {
    if (widget.attachment.typedStatus != AttachmentStatus.available) return;
    setState(() => _loading = true);
    try {
      final path = await widget.materialize();
      if (!mounted || path == null) return;
      await _playback.toggle(widget.attachment.id, path);
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final attachment = widget.attachment;
    final available = attachment.typedStatus == AttachmentStatus.available;
    final failed = attachment.typedStatus == AttachmentStatus.failed;
    final active = _playback.activeId == attachment.id;
    final duration = active && _playback.duration > Duration.zero
        ? _playback.duration
        : Duration.zero;
    final position = active ? _playback.position : Duration.zero;
    final progress = duration == Duration.zero
        ? 0.0
        : (position.inMilliseconds / duration.inMilliseconds).clamp(0.0, 1.0);
    final transferProgress = attachment.size <= 0
        ? 0.0
        : (attachment.offset / attachment.size).clamp(0.0, 1.0);

    return Container(
      margin: const EdgeInsets.only(top: 8),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: Theme.of(
          context,
        ).colorScheme.secondaryContainer.withValues(alpha: 0.35),
        borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
        border: Border.all(color: Theme.of(context).colorScheme.outlineVariant),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(context.torcaIcons.audio, size: 20),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  context.l10n.voiceMessage,
                  style: Theme.of(context).textTheme.labelLarge,
                ),
              ),
              Text(formatBytes(attachment.size)),
            ],
          ),
          const SizedBox(height: 8),
          Row(
            children: <Widget>[
              IconButton.filledTonal(
                tooltip: available
                    ? context.l10n.playVoiceMessage
                    : context.l10n.waitingForPeer,
                onPressed: available && !_loading ? _toggle : null,
                icon: _loading
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Icon(
                        active && _playback.playing
                            ? context.torcaIcons.pause
                            : context.torcaIcons.play,
                      ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: _VoiceWaveformProgress(
                  progress: active
                      ? progress
                      : (available ? 0 : transferProgress),
                  active: active,
                ),
              ),
              const SizedBox(width: 8),
              Text(_timeLabel(active ? position : Duration.zero)),
            ],
          ),
          const SizedBox(height: 4),
          Text(
            available
                ? (_playback.wasPlayed(attachment.id)
                      ? context.l10n.voiceMessagePlayed
                      : context.l10n.voiceMessageReady)
                : _statusLabel(context, attachment),
            style: Theme.of(context).textTheme.labelSmall,
          ),
          if (!available && !failed) ...<Widget>[
            const SizedBox(height: 4),
            LinearProgressIndicator(value: transferProgress),
          ],
          if (failed || !available)
            Wrap(
              spacing: 6,
              children: <Widget>[
                if (failed)
                  TextButton.icon(
                    onPressed: widget.operationBusy ? null : widget.onRetry,
                    icon: Icon(context.torcaIcons.retry),
                    label: Text(context.l10n.retryNow),
                  ),
                if (!available)
                  TextButton.icon(
                    onPressed: widget.operationBusy ? null : widget.onCancel,
                    icon: Icon(context.torcaIcons.close),
                    label: Text(context.l10n.cancel),
                  ),
              ],
            ),
        ],
      ),
    );
  }

  String _timeLabel(Duration value) =>
      '${value.inMinutes.toString().padLeft(2, '0')}:${(value.inSeconds % 60).toString().padLeft(2, '0')}';

  String _statusLabel(BuildContext context, AttachmentDto attachment) =>
      attachment.typedDirection == AttachmentDirection.inbound
      ? context.l10n.receivingSecurely
      : context.l10n.sendingSecurely;
}

class _VoiceWaveformProgress extends StatelessWidget {
  const _VoiceWaveformProgress({required this.progress, required this.active});
  final double progress;
  final bool active;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      const bars = 32;
      final width = constraints.maxWidth / bars;
      return SizedBox(
        height: 28,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: List<Widget>.generate(bars, (index) {
            final selected = index / bars <= progress;
            final height = 5 + ((index * 17) % 19).toDouble();
            return Container(
              width: width - 2,
              height: height,
              margin: const EdgeInsets.symmetric(horizontal: 1),
              decoration: BoxDecoration(
                color: selected
                    ? Theme.of(context).colorScheme.primary
                    : Theme.of(context).colorScheme.outlineVariant,
                borderRadius: BorderRadius.circular(
                  context.torcaTokens.radiusSmall,
                ),
              ),
            );
          }),
        ),
      );
    },
  );
}
