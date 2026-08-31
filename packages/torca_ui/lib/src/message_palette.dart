import 'package:flutter/material.dart';

import 'semantic_colors.dart';

/// Stable, accessible colors for message authors.
///
/// The key is an identity/contact id rather than a list position, so the same
/// author keeps the same accent after snapshots, restarts and sorting changes.
abstract final class TorcaMessagePalette {
  static ({
    Color surface,
    Color header,
    Color body,
    Color footer,
    Color connector,
    Color border,
    Color headerForeground,
    Color foreground,
    Color muted,
  })
  resolve(BuildContext context, String key, {required bool outbound}) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final semantic = theme.extension<TorcaSemanticColors>();
    final background = semantic?.chatBackground ?? scheme.surface;
    final directionBase = outbound
        ? semantic?.messageOutbound ?? scheme.primaryContainer
        : semantic?.messageInbound ?? scheme.surfaceContainerHighest;
    // Identity only nudges a theme-owned direction color. This keeps every
    // author deterministic without importing foreign hues into Gruvbox,
    // Tokyo Night, Forest, Graphite or the other appearance palettes.
    final identityStep = _stableIndex(key, 5);
    final identityMix = .04 + identityStep * .015;
    final identityAccent = outbound ? scheme.primary : scheme.tertiary;
    final body = Color.lerp(directionBase, identityAccent, identityMix)!;
    // The three surfaces form one message. Their contrast is deliberately
    // close: content dominates while the metadata bars remain subordinate.
    final header = Color.lerp(body, scheme.onSurface, .10)!;
    final footer = Color.lerp(body, background, .09)!;
    final surface = body;
    final headerForeground = _foregroundFor(header);
    final foreground = _foregroundFor(surface);
    final footerForeground = _foregroundFor(footer);
    return (
      surface: surface,
      header: header,
      body: body,
      footer: footer,
      connector: Color.lerp(body, scheme.onSurface, .16)!,
      border: Color.lerp(body, scheme.onSurface, .20)!,
      headerForeground: headerForeground,
      foreground: foreground,
      muted: footerForeground.withValues(alpha: .76),
    );
  }

  static Color _foregroundFor(Color color) =>
      ThemeData.estimateBrightnessForColor(color) == Brightness.dark
      ? Colors.white
      : Colors.black87;

  static int _stableIndex(String value, int length) {
    var hash = 0x811c9dc5;
    for (final unit in value.codeUnits) {
      hash ^= unit;
      hash = (hash * 0x01000193) & 0x7fffffff;
    }
    return hash % length;
  }
}
