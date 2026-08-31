import 'package:flutter/material.dart';

import 'semantic_colors.dart';

/// Stable, accessible colors for message authors.
///
/// The key is an identity/contact id rather than a list position, so the same
/// author keeps the same accent after snapshots, restarts and sorting changes.
abstract final class TorcaMessagePalette {
  // Direction-specific families guarantee that both sides of a conversation
  // remain distinguishable even when their stable identity hashes collide.
  static const List<Color> _outboundAccents = <Color>[
    Color(0xFF229ED9),
    Color(0xFF526ED3),
    Color(0xFF7656B5),
  ];
  static const List<Color> _inboundAccents = <Color>[
    Color(0xFF2E9D72),
    Color(0xFFB7791F),
    Color(0xFFD05A6E),
  ];

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
    final accents = outbound ? _outboundAccents : _inboundAccents;
    final index = _stableIndex(key, accents.length);
    final accent = accents[index];
    final background =
        theme.extension<TorcaSemanticColors>()?.chatBackground ??
        scheme.surface;
    // Message cards are intentionally built from three solid surfaces.  The
    // accent is identity-specific, while the blend levels keep the hierarchy
    // legible in both light and dark themes without relying on a card border.
    final header = Color.alphaBlend(accent.withValues(alpha: .82), background);
    final body = Color.alphaBlend(accent.withValues(alpha: .28), background);
    final footer = Color.alphaBlend(accent.withValues(alpha: .16), background);
    final surface = body;
    final headerForeground =
        ThemeData.estimateBrightnessForColor(header) == Brightness.dark
        ? Colors.white
        : Colors.black87;
    final foreground =
        ThemeData.estimateBrightnessForColor(surface) == Brightness.dark
        ? Colors.white
        : Colors.black87;
    return (
      surface: surface,
      header: header,
      body: body,
      footer: footer,
      connector: accent.withValues(alpha: .78),
      border: accent,
      headerForeground: headerForeground,
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
