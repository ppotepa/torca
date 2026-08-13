import 'package:flutter/services.dart';

/// A best-effort frame extractor.  The shared attachment package deliberately
/// knows nothing about platform codecs; unsupported files simply return null.
abstract final class VideoThumbnailService {
  static const MethodChannel _channel = MethodChannel('torca/media');

  static Future<Uint8List?> extract(String sourcePath) async {
    try {
      return await _channel.invokeMethod<Uint8List>(
        'videoThumbnail',
        <String, Object?>{'sourcePath': sourcePath},
      );
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }
}
