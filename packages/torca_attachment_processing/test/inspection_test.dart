import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as image;
import 'package:torca_attachment_processing/torca_attachment_processing.dart';

void main() {
  test('content signature wins over a misleading extension', () {
    final result = inspectAttachment(
      Uint8List.fromList(const <int>[0x25, 0x50, 0x44, 0x46, 0x2d]),
      'jpg',
    );
    expect(result.mediaType, 'application/pdf');
    expect(result.kind, AttachmentMediaKind.pdf);
  });

  test('classifies common document and archive extensions', () {
    expect(
      inspectAttachment(Uint8List(0), 'docx').kind,
      AttachmentMediaKind.document,
    );
    expect(
      inspectAttachment(Uint8List(0), 'zip').kind,
      AttachmentMediaKind.archive,
    );
  });

  test('keeps an OOXML document identity despite its ZIP signature', () {
    final result = inspectAttachment(
      Uint8List.fromList(const <int>[0x50, 0x4b, 0x03, 0x04]),
      'docx',
    );
    expect(result.kind, AttachmentMediaKind.document);
    expect(result.mediaType, contains('wordprocessingml'));
  });

  test('rejects shortcuts and executable payloads before queueing', () async {
    final source = File(
      '${Directory.systemTemp.path}${Platform.pathSeparator}'
      'torca-processing-executable-${DateTime.now().microsecondsSinceEpoch}.bin',
    );
    await source.writeAsBytes(const <int>[0x4d, 0x5a, 0, 0]);
    addTearDown(() async {
      if (await source.exists()) await source.delete();
    });

    await expectLater(
      const AttachmentProcessor().prepare(
        sourcePath: source.path,
        originalName: 'looks-safe.txt',
      ),
      throwsA(isA<AttachmentSelectionException>()),
    );
  });

  test(
    'rejects oversized non-image files before creating a staging copy',
    () async {
      final source = File(
        '${Directory.systemTemp.path}${Platform.pathSeparator}'
        'torca-processing-oversized-${DateTime.now().microsecondsSinceEpoch}.txt',
      );
      await source.writeAsString('too large');
      addTearDown(() async {
        if (await source.exists()) await source.delete();
      });

      await expectLater(
        const AttachmentProcessor().prepare(
          sourcePath: source.path,
          originalName: 'notes.txt',
          maximumBytes: 2,
        ),
        throwsA(isA<AttachmentSizeException>()),
      );
    },
  );

  test('prepares an image below the configured transport budget', () async {
    final source = image.Image(width: 960, height: 720);
    for (var y = 0; y < source.height; y += 1) {
      for (var x = 0; x < source.width; x += 1) {
        source.setPixelRgb(x, y, x % 256, y % 256, (x + y) % 256);
      }
    }
    final input = File(
      '${Directory.systemTemp.path}${Platform.pathSeparator}'
      'torca-processing-test-${DateTime.now().microsecondsSinceEpoch}.png',
    );
    await input.writeAsBytes(image.encodePng(source));
    addTearDown(() async {
      if (await input.exists()) await input.delete();
    });

    final prepared = await const AttachmentProcessor().prepare(
      sourcePath: input.path,
      originalName: 'photo.png',
    );
    expect(prepared.transformed, isTrue);
    expect(prepared.name, 'photo.png');
    expect(prepared.mediaType, 'image/jpeg');
    expect(prepared.size, lessThanOrEqualTo(50 * 1024));
    expect(await File(prepared.path).exists(), isTrue);
    expect(prepared.previewPath, isNotNull);
    expect(
      await File(prepared.previewPath!).length(),
      lessThanOrEqualTo(24 * 1024),
    );
    await prepared.dispose();
    expect(await File(prepared.path).exists(), isFalse);
    expect(await File(prepared.previewPath!).exists(), isFalse);
  });

  test(
    'stages non-image files so picker paths remain valid after selection',
    () async {
      final input = File(
        '${Directory.systemTemp.path}${Platform.pathSeparator}'
        'torca-processing-document-${DateTime.now().microsecondsSinceEpoch}.txt',
      );
      await input.writeAsString('stable staging payload');
      addTearDown(() async {
        if (await input.exists()) await input.delete();
      });

      final prepared = await const AttachmentProcessor().prepare(
        sourcePath: input.path,
        originalName: 'notes.txt',
      );
      expect(prepared.transformed, isFalse);
      expect(prepared.path, isNot(input.path));
      expect(
        await File(prepared.path).readAsString(),
        'stable staging payload',
      );
      await prepared.dispose();
      expect(await File(prepared.path).exists(), isFalse);
    },
  );

  test(
    'stores a best-effort video cover separately from the payload',
    () async {
      final input = File(
        '${Directory.systemTemp.path}${Platform.pathSeparator}'
        'torca-processing-video-${DateTime.now().microsecondsSinceEpoch}.mp4',
      );
      // The MP4 ftyp marker is enough for classification; decoding belongs to
      // the injected platform extractor rather than this package.
      await input.writeAsBytes(<int>[
        0,
        0,
        0,
        24,
        ...'ftypisom'.codeUnits,
        ...List<int>.filled(32, 0),
      ]);
      final cover = image.fill(
        image.Image(width: 32, height: 24),
        color: image.ColorRgb8(40, 80, 120),
      );
      final coverBytes = Uint8List.fromList(image.encodeJpg(cover));
      addTearDown(() async {
        if (await input.exists()) await input.delete();
      });

      final prepared = await const AttachmentProcessor().prepare(
        sourcePath: input.path,
        originalName: 'clip.mp4',
        videoPreviewExtractor: (_) async => coverBytes,
      );
      expect(prepared.kind, AttachmentMediaKind.video);
      expect(prepared.previewPath, isNotNull);
      expect(
        await File(prepared.previewPath!).length(),
        lessThanOrEqualTo(24 * 1024),
      );
      await prepared.dispose();
      expect(await File(prepared.previewPath!).exists(), isFalse);
    },
  );
}
