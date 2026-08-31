import 'package:flutter/material.dart';

import 'semantic_colors.dart';

/// Stable, accessible colors for message authors.
///
/// The key is an identity/contact id rather than a list position, so the same
/// author keeps the same accent after snapshots, restarts and sorting changes.
abstract final class TorcaMessagePalette {
  static const List<Color> _accents = <Color>[
    Color(0xFF229ED9),
    Color(0xFF2E9D72),
    Color(0xFFB7791F),
    Color(0xFF9B59B6),
    Color(0xFFD05A6E),
  ];

  static ({Color surface, Color border, Color foreground, Color muted}) resolve(
    BuildContext context,
    String key,
  ) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final index = _stableIndex(key, _accents.length);
    final accent = _accents[index];
    final background =
        theme.extension<TorcaSemanticColors>()?.chatBackground ??
        scheme.surface;
    final surface = Color.alphaBlend(accent.withValues(alpha: .34), background);
    final foreground =
        ThemeData.estimateBrightnessForColor(surface) == Brightness.dark
        ? Colors.white
        : Colors.black87;
    return (
      surface: surface,
      border: accent,
      foreground: foreground,
      muted: foreground.withValues(alpha: .72),
    );
  }

  static int _stableIndex(String value, int length) {
    var hash = 0x811c9dc5;
    for (final unit in value.codeUnits) {
      hash ^= unit;
      hash = (hash * 0x01000193) & 0x7fffffff;
    }
    return hash % length;
  }
}
