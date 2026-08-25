library;

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:image/image.dart' as image;

enum AttachmentMediaKind {
  image,
  video,
  audio,
  pdf,
  document,
  archive,
  text,
  binary,
}

/// A local file was deliberately refused before it reaches the encrypted
/// transfer queue.  This is distinct from a damaged/unsupported media file so
/// the UI can tell the user what action is safe.
class AttachmentSelectionException implements Exception {
  const AttachmentSelectionException(this.message);

  final String message;

  @override
  String toString() => message;
}

class AttachmentSizeException extends AttachmentSelectionException {
  const AttachmentSizeException({required this.maximumBytes})
    : super('Attachment exceeds the configured size limit.');

  final int maximumBytes;
}

class AttachmentProcessingPolicy {
  const AttachmentProcessingPolicy({
    this.targetImageBytes = 50 * 1024,
    this.targetPreviewBytes = 24 * 1024,
    this.maximumImageEdge = 1280,
    this.maximumPreviewEdge = 320,
    this.minimumImageEdge = 160,
    this.maximumSourceBytes = 64 * 1024 * 1024,
  });

  final int targetImageBytes;
  final int targetPreviewBytes;
  final int maximumImageEdge;
  final int maximumPreviewEdge;
  final int minimumImageEdge;
  final int maximumSourceBytes;
}

class AttachmentInspection {
  const AttachmentInspection({required this.mediaType, required this.kind});

  final String mediaType;
  final AttachmentMediaKind kind;
}

class PreparedAttachment {
  const PreparedAttachment({
    required this.path,
    required this.name,
    required this.mediaType,
    required this.kind,
    required this.size,
    required this.transformed,
    this.cleanupPath,
    this.previewPath,
  });

  final String path;
  final String name;
  final String mediaType;
  final AttachmentMediaKind kind;
  final int size;
  final bool transformed;

  /// A picker-provided path is not guaranteed to remain readable after the
  /// picker closes (especially on Android content providers).  When present,
  /// this app-owned staging file is removed after the native queue has copied
  /// it into encrypted storage.
  final String? cleanupPath;

  /// App-owned JPEG preview for metadata transfer.  It is independent from
  /// [path], whose bytes remain the complete attachment payload.
  final String? previewPath;

  Future<void> dispose() async {
    final ownedPaths = <String>{
      if (cleanupPath != null) cleanupPath!,
      if (cleanupPath == null && transformed) path,
      if (previewPath != null) previewPath!,
    };
    for (final ownedPath in ownedPaths) {
      final file = File(ownedPath);
      if (await file.exists()) await file.delete();
    }
  }

  /// The native command acknowledges queue admission before its attachment
  /// worker has opened the source. Keep the app-owned staging lease alive for
  /// the maximum normal job start window; deleting it immediately races the
  /// worker and leaves a durable job stuck at offset 0.
  Future<void> disposeAfter(Duration grace) async {
    await Future<void>.delayed(grace);
    await dispose();
  }
}

/// Removes abandoned app-owned staging files left behind when the process is
/// killed while a native attachment job is still being admitted.  The scan is
/// intentionally bounded to Torca's own filename prefixes and only deletes
/// files older than the lease window.
Future<int> cleanupStaleAttachmentStaging({
  Duration maxAge = const Duration(hours: 24),
}) async {
  final cutoff = DateTime.now().subtract(maxAge);
  var removed = 0;
  try {
    await for (final entity in Directory.systemTemp.list(followLinks: false)) {
      if (entity is! File) continue;
      final name = entity.uri.pathSegments.last;
      if (!_stagingPrefixes.any(name.startsWith)) continue;
      try {
        if ((await entity.stat()).modified.isBefore(cutoff)) {
          await entity.delete();
          removed++;
        }
      } on FileSystemException {
        // A concurrent native worker may still own the file. It will be
        // revisited on the next startup rather than interrupting the UI.
      }
    }
  } on FileSystemException {
    // Temporary storage is optional; attachment processing itself remains
    // available when the directory cannot be enumerated.
  }
  return removed;
}

const _stagingPrefixes = <String>[
  'torca-attachment-',
  'torca-image-',
  'torca-preview-',
  'torca-video-preview-',
  'torca-picked-',
];

class AttachmentProcessor {
  const AttachmentProcessor({this.policy = const AttachmentProcessingPolicy()});

  final AttachmentProcessingPolicy policy;

  Future<PreparedAttachment> prepare({
    required String sourcePath,
    required String originalName,
    String? extension,
    int? maximumBytes,
    int? maximumVideoBytes,
    VideoPreviewExtractor? videoPreviewExtractor,
  }) async {
    final sourceFile = File(sourcePath);
    final size = await sourceFile.length();
    if (size <= 0 || size > policy.maximumSourceBytes) {
      throw const FileSystemException('Attachment source size is invalid');
    }
    final prefix = await _readPrefix(sourceFile, 64);
    final inspection = inspectAttachment(
      prefix,
      extension ?? _extension(originalName),
    );
    final selectedLimit = inspection.kind == AttachmentMediaKind.video
        ? maximumVideoBytes
        : maximumBytes;
    if (inspection.kind != AttachmentMediaKind.image &&
        selectedLimit != null &&
        size > selectedLimit) {
      throw AttachmentSizeException(maximumBytes: selectedLimit);
    }
    final normalizedExtension = (extension ?? _extension(originalName) ?? '')
        .replaceFirst('.', '')
        .toLowerCase();
    if (_blockedExtensions.contains(normalizedExtension) ||
        _isExecutable(prefix)) {
      throw const AttachmentSelectionException(
        'Executable files and desktop shortcuts cannot be sent.',
      );
    }
    if (inspection.kind != AttachmentMediaKind.image) {
      // Do not retain a URI/path owned by file_picker.  The native runtime may
      // process this job after the picker route has gone away, so make an
      // app-owned copy for every non-image attachment too.
      final staged = File(
        '${Directory.systemTemp.path}${Platform.pathSeparator}'
        'torca-attachment-${DateTime.now().microsecondsSinceEpoch}.$normalizedExtension',
      );
      await sourceFile.copy(staged.path);
      final previewPath = inspection.kind == AttachmentMediaKind.video
          ? await _createVideoPreview(staged.path, videoPreviewExtractor)
          : null;
      return PreparedAttachment(
        path: staged.path,
        name: originalName,
        mediaType: inspection.mediaType,
        kind: inspection.kind,
        size: size,
        transformed: false,
        cleanupPath: staged.path,
        previewPath: previewPath,
      );
    }

    final source = await sourceFile.readAsBytes();
    final processed = await compute(_processImage, _ImageJob(source, policy));
    final target = File(
      '${Directory.systemTemp.path}${Platform.pathSeparator}'
      'torca-image-${DateTime.now().microsecondsSinceEpoch}.jpg',
    );
    await target.writeAsBytes(processed.payload, flush: true);
    final preview = File(
      '${Directory.systemTemp.path}${Platform.pathSeparator}'
      'torca-preview-${DateTime.now().microsecondsSinceEpoch}.jpg',
    );
    await preview.writeAsBytes(processed.preview, flush: true);
    return PreparedAttachment(
      path: target.path,
      // The media type describes the bytes on the wire; the name is a user
      // label and must survive optimisation.  Renaming a selected photo to a
      // generated `.jpg` made the sender and receiver lose the file identity
      // the user deliberately chose.
      name: originalName,
      mediaType: 'image/jpeg',
      kind: AttachmentMediaKind.image,
      size: processed.payload.length,
      transformed: true,
      previewPath: preview.path,
    );
  }

  Future<String?> _createVideoPreview(
    String stagedVideoPath,
    VideoPreviewExtractor? extractor,
  ) async {
    if (extractor == null) return null;
    try {
      final bytes = await extractor(stagedVideoPath);
      if (bytes == null ||
          bytes.isEmpty ||
          bytes.length > policy.targetPreviewBytes) {
        return null;
      }
      final decoded = image.decodeImage(bytes);
      if (decoded == null) return null;
      final preview = _encodePreview(decoded, policy);
      final target = File(
        '${Directory.systemTemp.path}${Platform.pathSeparator}'
        'torca-video-preview-${DateTime.now().microsecondsSinceEpoch}.jpg',
      );
      await target.writeAsBytes(preview, flush: true);
      return target.path;
    } on Object {
      // Preview extraction is purely progressive enhancement.  A platform
      // decoder must never make an otherwise valid attachment unsendable.
      return null;
    }
  }
}

/// Platform-owned extraction keeps codec dependencies out of the shared
/// processor.  Callers may return null when a platform or container is not
/// supported; the attachment then uses the normal video card fallback.
typedef VideoPreviewExtractor = Future<Uint8List?> Function(String sourcePath);

const _blockedExtensions = <String>{
  // A shortcut is data, not the target selected by the user.  Never resolve it
  // implicitly: that could leak a different local file.
  'lnk',
  'url',
  // Executables and script launchers are rejected until an explicit, reviewed
  // product policy is introduced.
  'exe',
  'msi',
  'com',
  'scr',
  'bat',
  'cmd',
  'ps1',
  'vbs',
  'js',
  'jar',
  'apk',
};

bool _isExecutable(Uint8List prefix) =>
    prefix.length >= 2 && prefix[0] == 0x4d && prefix[1] == 0x5a;

AttachmentInspection inspectAttachment(Uint8List prefix, String? extension) {
  bool starts(List<int> signature) {
    if (prefix.length < signature.length) return false;
    for (var index = 0; index < signature.length; index += 1) {
      if (prefix[index] != signature[index]) return false;
    }
    return true;
  }

  bool asciiAt(int offset, String value) =>
      prefix.length >= offset + value.length &&
      String.fromCharCodes(prefix.sublist(offset, offset + value.length)) ==
          value;

  final ext = (extension ?? '').replaceFirst('.', '').toLowerCase();

  if (starts(const <int>[0xff, 0xd8, 0xff])) return _type('image/jpeg');
  if (starts(const <int>[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) {
    return _type('image/png');
  }
  if (asciiAt(0, 'GIF87a') || asciiAt(0, 'GIF89a')) return _type('image/gif');
  if (asciiAt(0, 'RIFF') && asciiAt(8, 'WEBP')) return _type('image/webp');
  if (asciiAt(0, '%PDF-')) return _type('application/pdf');
  if (starts(const <int>[0x50, 0x4b, 0x03, 0x04])) {
    // Modern Office documents are ZIP containers. Preserve the user-visible
    // document type instead of flattening every OOXML file into an archive.
    return _type(switch (ext) {
      'docx' =>
        'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      'xlsx' =>
        'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      'pptx' =>
        'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      _ => 'application/zip',
    });
  }
  if (starts(const <int>[0x1f, 0x8b])) return _type('application/gzip');
  if (prefix.length >= 12 && asciiAt(4, 'ftyp')) {
    return _type(switch (ext) {
      'm4a' || 'm4b' || 'm4p' => 'audio/mp4',
      _ => 'video/mp4',
    });
  }
  if (asciiAt(0, 'ID3')) return _type('audio/mpeg');
  if (asciiAt(0, 'OggS')) return _type('audio/ogg');
  if (asciiAt(0, 'RIFF') && asciiAt(8, 'WAVE')) return _type('audio/wav');

  return _type(switch (ext) {
    'jpg' || 'jpeg' => 'image/jpeg',
    'png' => 'image/png',
    'gif' => 'image/gif',
    'webp' => 'image/webp',
    'pdf' => 'application/pdf',
    'txt' || 'md' || 'log' => 'text/plain',
    'json' => 'application/json',
    'csv' => 'text/csv',
    'zip' => 'application/zip',
    'gz' || 'gzip' => 'application/gzip',
    'doc' => 'application/msword',
    'docx' =>
      'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    'xls' => 'application/vnd.ms-excel',
    'xlsx' =>
      'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    'ppt' => 'application/vnd.ms-powerpoint',
    'pptx' =>
      'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    'mp4' || 'm4v' => 'video/mp4',
    'webm' => 'video/webm',
    'mp3' => 'audio/mpeg',
    'm4a' || 'm4b' || 'm4p' => 'audio/mp4',
    'wav' => 'audio/wav',
    'ogg' || 'oga' => 'audio/ogg',
    _ => 'application/octet-stream',
  });
}

AttachmentInspection _type(String mediaType) => AttachmentInspection(
  mediaType: mediaType,
  kind: mediaType.startsWith('image/')
      ? AttachmentMediaKind.image
      : mediaType.startsWith('video/')
      ? AttachmentMediaKind.video
      : mediaType.startsWith('audio/')
      ? AttachmentMediaKind.audio
      : mediaType == 'application/pdf'
      ? AttachmentMediaKind.pdf
      : mediaType.contains('zip') || mediaType.contains('gzip')
      ? AttachmentMediaKind.archive
      : mediaType.startsWith('text/') || mediaType == 'application/json'
      ? AttachmentMediaKind.text
      : mediaType.contains('word') ||
            mediaType.contains('excel') ||
            mediaType.contains('powerpoint') ||
            mediaType.contains('officedocument')
      ? AttachmentMediaKind.document
      : AttachmentMediaKind.binary,
);

Future<Uint8List> _readPrefix(File file, int maximum) async {
  final input = await file.open();
  try {
    // Await before closing the handle; returning the Future directly lets the
    // finally block race the pending Windows file read.
    return await input.read(maximum);
  } finally {
    await input.close();
  }
}

String? _extension(String name) {
  final dot = name.lastIndexOf('.');
  return dot < 0 || dot == name.length - 1 ? null : name.substring(dot + 1);
}

class _ImageJob {
  const _ImageJob(this.bytes, this.policy);
  final Uint8List bytes;
  final AttachmentProcessingPolicy policy;
}

class _ProcessedImage {
  const _ProcessedImage(this.payload, this.preview);
  final Uint8List payload;
  final Uint8List preview;
}

_ProcessedImage _processImage(_ImageJob job) {
  final source = image.decodeImage(job.bytes);
  if (source == null) throw const FormatException('Unsupported image');
  var decoded = image.bakeOrientation(source);
  if (decoded.width > job.policy.maximumImageEdge ||
      decoded.height > job.policy.maximumImageEdge) {
    final landscape = decoded.width >= decoded.height;
    decoded = image.copyResize(
      decoded,
      width: landscape ? job.policy.maximumImageEdge : null,
      height: landscape ? null : job.policy.maximumImageEdge,
      interpolation: image.Interpolation.average,
    );
  }
  while (true) {
    for (final quality in const <int>[82, 72, 62, 52, 42, 34, 28]) {
      final encoded = image.encodeJpg(decoded, quality: quality);
      if (encoded.length <= job.policy.targetImageBytes) {
        return _ProcessedImage(
          Uint8List.fromList(encoded),
          _encodePreview(decoded, job.policy),
        );
      }
    }
    if (decoded.width <= 240 && decoded.height <= 240) {
      throw const FileSystemException('Image cannot fit the configured limit');
    }
    final landscape = decoded.width >= decoded.height;
    final longEdge = landscape ? decoded.width : decoded.height;
    final lowerBound = job.policy.minimumImageEdge.clamp(1, longEdge - 1);
    final nextLongEdge = (longEdge * .78).round().clamp(
      lowerBound,
      longEdge - 1,
    );
    decoded = image.copyResize(
      decoded,
      width: landscape ? nextLongEdge : null,
      height: landscape ? null : nextLongEdge,
      interpolation: image.Interpolation.average,
    );
  }
}

Uint8List _encodePreview(
  image.Image source,
  AttachmentProcessingPolicy policy,
) {
  final landscape = source.width >= source.height;
  var preview = image.copyResize(
    source,
    width: landscape ? policy.maximumPreviewEdge : null,
    height: landscape ? null : policy.maximumPreviewEdge,
    interpolation: image.Interpolation.average,
  );
  for (final quality in const <int>[74, 62, 50, 40, 32]) {
    final encoded = image.encodeJpg(preview, quality: quality);
    if (encoded.length <= policy.targetPreviewBytes) {
      return Uint8List.fromList(encoded);
    }
  }
  preview = image.copyResize(
    preview,
    width: landscape ? 160 : null,
    height: landscape ? null : 160,
    interpolation: image.Interpolation.average,
  );
  return Uint8List.fromList(image.encodeJpg(preview, quality: 28));
}
