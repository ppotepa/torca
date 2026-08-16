import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:record/record.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';

const Duration voiceClipMaximumDuration = Duration(seconds: 10);

typedef VoiceClipReady = Future<void> Function(String path, String name);

/// Records one short voice note and hands the resulting file to the regular
/// attachment pipeline. Transport, retry, progress and persistence therefore
/// remain owned by attachment jobs instead of a second audio upload path.
class VoiceClipRecorder extends StatefulWidget {
  const VoiceClipRecorder({
    required this.onClipReady,
    this.disabled = false,
    this.recorderFactory = _defaultRecorderFactory,
    super.key,
  });

  final VoiceClipReady onClipReady;
  final bool disabled;
  final AudioClipRecorder Function() recorderFactory;

  @override
  State<VoiceClipRecorder> createState() => _VoiceClipRecorderState();
}

abstract interface class AudioClipRecorder {
  Future<bool> start(String path);
  Future<String?> stop();
  Future<void> cancel();
  Future<void> dispose();
}

AudioClipRecorder _defaultRecorderFactory() => _RecordAudioClipRecorder();

class _RecordAudioClipRecorder implements AudioClipRecorder {
  final AudioRecorder _recorder = AudioRecorder();

  @override
  Future<bool> start(String path) async {
    if (!await _recorder.hasPermission()) return false;
    await _recorder.start(
      const RecordConfig(
        encoder: AudioEncoder.aacLc,
        bitRate: 24000,
        sampleRate: 16000,
        numChannels: 1,
        autoGain: true,
        echoCancel: true,
        noiseSuppress: true,
      ),
      path: path,
    );
    return true;
  }

  @override
  Future<String?> stop() => _recorder.stop();

  @override
  Future<void> cancel() => _recorder.cancel();

  @override
  Future<void> dispose() => _recorder.dispose();
}

class _VoiceClipRecorderState extends State<VoiceClipRecorder>
    with WidgetsBindingObserver {
  late final AudioClipRecorder _recorder;
  Timer? _limitTimer;
  Timer? _elapsedTimer;
  bool _pointerHeld = false;
  bool _starting = false;
  bool _recording = false;
  bool _finishing = false;
  Duration _elapsed = Duration.zero;
  String? _pendingPath;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _recorder = widget.recorderFactory();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed) unawaited(_cancel());
  }

  Future<void> _start() async {
    if (widget.disabled || _starting || _recording || _finishing) return;
    _pointerHeld = true;
    _starting = true;
    final stamp = DateTime.now();
    final name =
        'voice-${stamp.year.toString().padLeft(4, '0')}'
        '${stamp.month.toString().padLeft(2, '0')}'
        '${stamp.day.toString().padLeft(2, '0')}-'
        '${stamp.hour.toString().padLeft(2, '0')}'
        '${stamp.minute.toString().padLeft(2, '0')}'
        '${stamp.second.toString().padLeft(2, '0')}.m4a';
    final path = '${Directory.systemTemp.path}${Platform.pathSeparator}$name';
    _pendingPath = path;
    try {
      final started = await _recorder.start(path);
      if (!mounted || !started) {
        await _discardFile(path);
        if (mounted) _showError(context.strings.microphonePermissionRequired);
        return;
      }
      _recording = true;
      _elapsed = Duration.zero;
      _limitTimer = Timer(voiceClipMaximumDuration, () => unawaited(_finish()));
      _elapsedTimer = Timer.periodic(const Duration(milliseconds: 100), (_) {
        if (!mounted || !_recording) return;
        setState(() {
          final next = _elapsed + const Duration(milliseconds: 100);
          _elapsed = next > voiceClipMaximumDuration
              ? voiceClipMaximumDuration
              : next;
        });
      });
      if (mounted) setState(() {});
      if (!_pointerHeld) await _finish();
    } on Object {
      await _recorder.cancel();
      await _discardFile(path);
      if (mounted) _showError(context.strings.voiceClipRecordingFailed);
    } finally {
      _starting = false;
      if (mounted) setState(() {});
    }
  }

  Future<void> _finish() async {
    _pointerHeld = false;
    if (!_recording || _finishing) return;
    _finishing = true;
    _recording = false;
    _stopTimers();
    try {
      final path = await _recorder.stop() ?? _pendingPath;
      if (path == null) return;
      final file = File(path);
      if (!await file.exists() || await file.length() == 0) {
        await _discardFile(path);
        if (mounted) _showError(context.strings.voiceClipRecordingFailed);
        return;
      }
      await widget.onClipReady(path, file.uri.pathSegments.last);
    } on Object {
      final path = _pendingPath;
      if (path != null) await _discardFile(path);
      if (mounted) _showError(context.strings.voiceClipRecordingFailed);
    } finally {
      _pendingPath = null;
      _finishing = false;
      if (mounted) setState(() => _elapsed = Duration.zero);
    }
  }

  Future<void> _cancel() async {
    _pointerHeld = false;
    if (!_recording && !_starting) return;
    _recording = false;
    _stopTimers();
    await _recorder.cancel();
    final path = _pendingPath;
    _pendingPath = null;
    if (path != null) await _discardFile(path);
    if (mounted) setState(() => _elapsed = Duration.zero);
  }

  void _stopTimers() {
    _limitTimer?.cancel();
    _limitTimer = null;
    _elapsedTimer?.cancel();
    _elapsedTimer = null;
  }

  Future<void> _discardFile(String path) async {
    final file = File(path);
    if (await file.exists()) await file.delete();
  }

  void _showError(String message) {
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _stopTimers();
    unawaited(_recorder.cancel());
    unawaited(_recorder.dispose());
    final path = _pendingPath;
    if (path != null) unawaited(_discardFile(path));
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final busy = _starting || _recording || _finishing;
    final secondsLeft = (voiceClipMaximumDuration - _elapsed).inSeconds + 1;
    return Semantics(
      button: true,
      label: _recording
          ? context.strings.voiceClipRecording(secondsLeft.clamp(0, 10))
          : context.strings.holdToRecordVoiceClip,
      child: Listener(
        behavior: HitTestBehavior.opaque,
        onPointerDown: widget.disabled ? null : (_) => unawaited(_start()),
        onPointerUp: widget.disabled ? null : (_) => unawaited(_finish()),
        onPointerCancel: widget.disabled ? null : (_) => unawaited(_cancel()),
        child: AnimatedContainer(
          key: const ValueKey<String>('voice-clip-recorder'),
          duration: const Duration(milliseconds: 160),
          width: 48,
          height: 48,
          decoration: BoxDecoration(
            color: _recording
                ? Theme.of(context).colorScheme.error
                : Theme.of(context).colorScheme.secondaryContainer,
            borderRadius: BorderRadius.circular(
              context.torcaTokens.radiusLarge,
            ),
          ),
          alignment: Alignment.center,
          child: _finishing
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Column(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Icon(
                      context.torcaIcons.pushToTalk,
                      size: busy ? 19 : 22,
                      color: _recording
                          ? Theme.of(context).colorScheme.onError
                          : Theme.of(context).colorScheme.onSecondaryContainer,
                    ),
                    if (_recording)
                      Text(
                        '${secondsLeft.clamp(0, 10)}',
                        style: Theme.of(context).textTheme.labelSmall?.copyWith(
                          color: Theme.of(context).colorScheme.onError,
                          height: 1,
                        ),
                      ),
                  ],
                ),
        ),
      ),
    );
  }
}
