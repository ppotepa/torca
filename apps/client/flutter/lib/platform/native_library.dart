import 'dart:io';

/// Native library naming is the only platform branch allowed in Dart.
String nativeRuntimeLibraryName() {
  if (Platform.isWindows) return 'torca_native.dll';
  if (Platform.isAndroid) return 'libtorca_native.so';
  if (Platform.isLinux) return 'libtorca_native.so';
  if (Platform.isMacOS || Platform.isIOS) return 'libtorca_native.dylib';
  throw UnsupportedError(
    'Torca native runtime is unsupported on this platform',
  );
}
