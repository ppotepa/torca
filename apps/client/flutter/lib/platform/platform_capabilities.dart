import 'dart:io';

bool get isTorcaAndroid => Platform.isAndroid;
bool get isTorcaWindows => Platform.isWindows;
bool get isTorcaDesktop =>
    Platform.isWindows || Platform.isLinux || Platform.isMacOS;
String get torcaPathSeparator => Platform.pathSeparator;
