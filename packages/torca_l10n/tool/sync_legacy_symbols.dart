import 'dart:io';

/// Regenerates the public localization API from the ARB catalogs.
void main() {
  final result = Process.runSync('flutter', ['gen-l10n']);
  stdout.write(result.stdout);
  stderr.write(result.stderr);
  exitCode = result.exitCode;
}
