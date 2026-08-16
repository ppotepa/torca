import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/voice_clip_recorder.dart';

class _FakeRecorder implements AudioClipRecorder {
  String? path;
  bool cancelled = false;
  bool disposed = false;

  @override
  Future<bool> start(String path) async {
    this.path = path;
    File(path).writeAsBytesSync(Uint8List.fromList(<int>[1, 2, 3]));
    return true;
  }

  @override
  Future<double?> amplitude() async => -60;

  @override
  Future<String?> stop() async => path;

  @override
  Future<void> cancel() async {
    cancelled = true;
  }

  @override
  Future<void> dispose() async {
    disposed = true;
  }
}

void main() {
  testWidgets('holding the audio action starts a bounded voice clip', (
    tester,
  ) async {
    final recorder = _FakeRecorder();
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: VoiceClipRecorder(
            recorderFactory: () => recorder,
            onClipReady: (_, _) async {},
          ),
        ),
      ),
    );

    final gesture = await tester.startGesture(
      tester.getCenter(
        find.byKey(const ValueKey<String>('voice-clip-recorder')),
      ),
    );
    await tester.pump(const Duration(milliseconds: 50));
    expect(recorder.path, endsWith('.m4a'));
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    await tester.pump();
    expect(recorder.cancelled, isTrue);
    await gesture.up();
  });

  testWidgets('leaving the app cancels an active voice clip', (tester) async {
    final recorder = _FakeRecorder();
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: VoiceClipRecorder(
            recorderFactory: () => recorder,
            onClipReady: (_, _) async {},
          ),
        ),
      ),
    );

    await tester.startGesture(
      tester.getCenter(
        find.byKey(const ValueKey<String>('voice-clip-recorder')),
      ),
    );
    await tester.pump(const Duration(milliseconds: 50));
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    await tester.pump();

    expect(recorder.cancelled, isTrue);
  });
}
