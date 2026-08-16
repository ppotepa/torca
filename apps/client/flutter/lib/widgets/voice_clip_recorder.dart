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
  Future<double?> amplitude();
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
        bitRate: 16000,
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
  Future<double?> amplitude() =>
      _recorder.getAmplitude().then((value) => value.current);

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
  double? _pointerStartX;
  final List<double> _waveform = <double>[];
  Timer? _amplitudeTimer;

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
      _waveform.clear();
      _amplitudeTimer = Timer.periodic(const Duration(milliseconds: 100), (_) {
        unawaited(_sampleAmplitude());
      });
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
    _amplitudeTimer?.cancel();
    _amplitudeTimer = null;
  }

  Future<void> _sampleAmplitude() async {
    if (!_recording || !mounted) return;
    // AudioRecorder exposes amplitude independently of the encoded file. Keep
    // only a compact rolling history so the overlay remains cheap to paint.
    double? amplitude;
    try {
      amplitude = await _recorder.amplitude();
    } on Object {
      return;
    }
    final currentAmplitude = amplitude;
    if (!mounted || !_recording || currentAmplitude == null) return;
    setState(() {
      _waveform.add(((currentAmplitude + 60) / 60).clamp(0.04, 1.0));
      if (_waveform.length > 64) _waveform.removeAt(0);
    });
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
        onPointerDown: widget.disabled
            ? null
            : (event) {
                _pointerStartX = event.position.dx;
                unawaited(_start());
              },
        onPointerMove: widget.disabled
            ? null
            : (event) {
                final start = _pointerStartX;
                if (start != null && start - event.position.dx > 60) {
                  unawaited(_cancel());
                }
              },
        onPointerUp: widget.disabled ? null : (_) => unawaited(_finish()),
        onPointerCancel: widget.disabled ? null : (_) => unawaited(_cancel()),
        child: Stack(
          clipBehavior: Clip.none,
          alignment: Alignment.bottomRight,
          children: <Widget>[
            if (_recording)
              Positioned(
                right: 0,
                bottom: 54,
                child: _RecordingOverlay(
                  waveform: List<double>.of(_waveform),
                  secondsLeft: secondsLeft.clamp(0, 10),
                ),
              ),
            AnimatedContainer(
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
                boxShadow: _recording
                    ? <BoxShadow>[
                        BoxShadow(
                          color: Theme.of(
                            context,
                          ).colorScheme.error.withValues(alpha: 0.42),
                          blurRadius: 16,
                          spreadRadius: 4,
                        ),
                      ]
                    : null,
              ),
              alignment: Alignment.center,
              child: _finishing
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Icon(
                      context.torcaIcons.pushToTalk,
                      size: busy ? 19 : 22,
                      color: _recording
                          ? Theme.of(context).colorScheme.onError
                          : Theme.of(context).colorScheme.onSecondaryContainer,
                    ),
            ),
          ],
        ),
      ),
    );
  }
}

class _WaveformPainter extends CustomPainter {
  const _WaveformPainter({required this.values, required this.color});
  final List<double> values;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..strokeWidth = 1.5
      ..strokeCap = StrokeCap.round;
    if (values.isEmpty) {
      canvas.drawLine(
        Offset(0, size.height / 2),
        Offset(size.width, size.height / 2),
        paint,
      );
      return;
    }
    final step = size.width / values.length;
    for (var index = 0; index < values.length; index++) {
      final height = values[index] * size.height;
      final x = (index + 0.5) * step;
      canvas.drawLine(
        Offset(x, (size.height - height) / 2),
        Offset(x, (size.height + height) / 2),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant _WaveformPainter oldDelegate) =>
      oldDelegate.values != values || oldDelegate.color != color;
}

class _RecordingOverlay extends StatelessWidget {
  const _RecordingOverlay({required this.waveform, required this.secondsLeft});
  final List<double> waveform;
  final int secondsLeft;

  @override
  Widget build(BuildContext context) => Material(
    elevation: 8,
    color: Theme.of(context).colorScheme.errorContainer,
    borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
    child: Container(
      width: 244,
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 9),
      decoration: BoxDecoration(
        border: Border.all(color: Theme.of(context).colorScheme.error),
        borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(
                context.torcaIcons.pushToTalk,
                color: Theme.of(context).colorScheme.onErrorContainer,
                size: 16,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  context.strings.voiceClipRecording(secondsLeft),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.labelLarge?.copyWith(
                    color: Theme.of(context).colorScheme.onErrorContainer,
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              Text(
                '${secondsLeft}s',
                style: Theme.of(context).textTheme.labelMedium?.copyWith(
                  color: Theme.of(context).colorScheme.onErrorContainer,
                ),
              ),
            ],
          ),
          const SizedBox(height: 5),
          SizedBox(
            height: 30,
            width: double.infinity,
            child: CustomPaint(
              painter: _WaveformPainter(
                values: waveform,
                color: Theme.of(context).colorScheme.onErrorContainer,
              ),
            ),
          ),
          const SizedBox(height: 3),
          Text(
            '← ${context.strings.cancel}',
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: Theme.of(context).colorScheme.onErrorContainer,
            ),
          ),
        ],
      ),
    ),
  );
}
