import 'dart:ffi' as ffi;
import 'dart:io';

typedef _ShutdownNative = ffi.Int32 Function();
typedef _ShutdownDart = int Function();

/// Stops the process-owned Rust runtime for an explicit application Quit.
/// Normal gateway disposal only releases presentation handles.
Future<void> shutdownProcessRuntime() async {
  if (!Platform.isWindows) return;
  final library = ffi.DynamicLibrary.open('torca_bridge.dll');
  final shutdown = library.lookupFunction<_ShutdownNative, _ShutdownDart>(
    'torca_process_shutdown',
  );
  final status = shutdown();
  if (status != 0) {
    throw StateError('Torca process runtime shutdown failed with status $status');
  }
}
