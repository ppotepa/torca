import 'package:flutter/material.dart';

import 'appearance.dart';
import 'icon_set.dart';
import 'semantic_colors.dart';
import 'tokens.dart';

abstract final class TorcaThemeFactory {
  static ThemeData build(TorcaAppearance appearance, Brightness brightness) {
    final palette = _palette(appearance.variant, brightness);
    final terminal = appearance.family == TorcaThemeFamily.terminal;
    final compact = appearance.density == TorcaDensity.compact;
    final radiusSmall = terminal ? 0.0 : 6.0;
    final radiusMedium = terminal ? 2.0 : 8.0;
    final radiusLarge = terminal ? 3.0 : 12.0;
    final scheme =
        ColorScheme.fromSeed(
          seedColor: palette.primary,
          brightness: brightness,
        ).copyWith(
          primary: palette.primary,
          secondary: palette.secondary,
          tertiary: palette.accent,
          error: palette.error,
          surface: palette.surface,
          onSurface: palette.onSurface,
          outline: palette.outline,
          outlineVariant: palette.outline.withValues(alpha: .48),
          surfaceContainerHighest: palette.container,
          primaryContainer: palette.primaryContainer,
        );
    final base = ThemeData(
      useMaterial3: true,
      brightness: brightness,
      colorScheme: scheme,
      scaffoldBackgroundColor: palette.background,
      visualDensity: compact ? VisualDensity.compact : VisualDensity.standard,
      materialTapTargetSize: compact
          ? MaterialTapTargetSize.shrinkWrap
          : MaterialTapTargetSize.padded,
      dividerColor: scheme.outlineVariant,
      focusColor: scheme.primary.withValues(alpha: .18),
    );
    final displayFamily = terminal ? 'PressStart2P' : null;
    final displayPackage = terminal ? 'torca_ui' : null;
    final text = base.textTheme.copyWith(
      headlineSmall: base.textTheme.headlineSmall?.copyWith(
        fontFamily: displayFamily,
        package: displayPackage,
        fontSize: terminal ? 16 : 24,
        height: terminal ? 1.5 : null,
      ),
      titleLarge: base.textTheme.titleLarge?.copyWith(
        fontFamily: displayFamily,
        package: displayPackage,
        fontSize: terminal ? 13 : 22,
        height: terminal ? 1.5 : null,
      ),
      titleMedium: base.textTheme.titleMedium?.copyWith(
        fontFamily: displayFamily,
        package: displayPackage,
        fontSize: terminal ? 11 : 16,
        height: terminal ? 1.45 : null,
      ),
      labelLarge: base.textTheme.labelLarge?.copyWith(
        fontFamily: displayFamily,
        package: displayPackage,
        fontSize: terminal ? 9 : 14,
      ),
    );
    final shape = RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(radiusMedium),
      side: terminal
          ? BorderSide(color: scheme.outline, width: 1)
          : BorderSide.none,
    );
    return base.copyWith(
      textTheme: text,
      appBarTheme: AppBarTheme(
        centerTitle: false,
        elevation: 0,
        scrolledUnderElevation: terminal ? 0 : 1,
        backgroundColor: palette.surface,
        foregroundColor: palette.onSurface,
        surfaceTintColor: Colors.transparent,
        shape: terminal
            ? Border(bottom: BorderSide(color: scheme.outline))
            : null,
      ),
      cardTheme: CardThemeData(
        clipBehavior: Clip.antiAlias,
        margin: EdgeInsets.zero,
        elevation: 0,
        color: palette.surface,
        shape: shape,
      ),
      dialogTheme: DialogThemeData(
        elevation: terminal ? 0 : 6,
        backgroundColor: palette.surface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusLarge),
          side: terminal
              ? BorderSide(color: scheme.primary, width: 2)
              : BorderSide.none,
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: palette.container,
        isDense: compact,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(radiusSmall),
          borderSide: BorderSide(color: scheme.outline),
        ),
      ),
      floatingActionButtonTheme: FloatingActionButtonThemeData(
        elevation: terminal ? 0 : 3,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusMedium),
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(shape: shape),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          shape: shape,
          side: BorderSide(color: scheme.outline),
        ),
      ),
      listTileTheme: ListTileThemeData(
        dense: compact,
        minTileHeight: compact ? 52 : 64,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusSmall),
        ),
      ),
      navigationBarTheme: NavigationBarThemeData(
        height: compact ? 62 : 72,
        elevation: 0,
        indicatorShape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(terminal ? 0 : 8),
        ),
      ),
      tooltipTheme: const TooltipThemeData(
        waitDuration: Duration(milliseconds: 450),
      ),
      extensions: <ThemeExtension<dynamic>>[
        TorcaTokens(
          terminal: terminal,
          compact: compact,
          radiusSmall: radiusSmall,
          radiusMedium: radiusMedium,
          radiusLarge: radiusLarge,
          spaceUnit: compact ? 4 : 6,
          listTileHeight: compact ? 52 : 64,
          borderWidth: terminal ? 1 : .5,
          animationDuration: appearance.reduceMotion
              ? Duration.zero
              : const Duration(milliseconds: 180),
        ),
        TorcaSemanticColors.fromScheme(scheme).copyWith(
          chatBackground: palette.background,
          separator: palette.outline,
        ),
        terminal ? TorcaIconSet.terminal : TorcaIconSet.modern,
      ],
    );
  }
}

class _Palette {
  const _Palette({
    required this.background,
    required this.surface,
    required this.container,
    required this.onSurface,
    required this.primary,
    required this.primaryContainer,
    required this.secondary,
    required this.accent,
    required this.error,
    required this.outline,
  });
  final Color background;
  final Color surface;
  final Color container;
  final Color onSurface;
  final Color primary;
  final Color primaryContainer;
  final Color secondary;
  final Color accent;
  final Color error;
  final Color outline;
}

_Palette _palette(TorcaThemeVariant variant, Brightness brightness) {
  final dark = brightness == Brightness.dark;
  return switch (variant) {
    TorcaThemeVariant.modernOcean => _modern(
      dark,
      const Color(0xFF229ED9),
      const Color(0xFF5BC0EB),
    ),
    TorcaThemeVariant.modernGraphite => _modern(
      dark,
      const Color(0xFF64748B),
      const Color(0xFF94A3B8),
    ),
    TorcaThemeVariant.modernForest => _modern(
      dark,
      const Color(0xFF2E7D68),
      const Color(0xFF64B59E),
    ),
    TorcaThemeVariant.terminalGruvbox =>
      dark
          ? _terminal(
              0xFF282828,
              0xFF3C3836,
              0xFF504945,
              0xFFEBDBB2,
              0xFFB8BB26,
              0xFF98971A,
              0xFFFABD2F,
              0xFF83A598,
              0xFFFB4934,
              0xFF665C54,
            )
          : _terminal(
              0xFFFBF1C7,
              0xFFF2E5BC,
              0xFFEBDBB2,
              0xFF3C3836,
              0xFF79740E,
              0xFFD5C4A1,
              0xFFB57614,
              0xFF076678,
              0xFF9D0006,
              0xFFBDAE93,
            ),
    TorcaThemeVariant.terminalDracula =>
      dark
          ? _terminal(
              0xFF282A36,
              0xFF30323F,
              0xFF44475A,
              0xFFF8F8F2,
              0xFFBD93F9,
              0xFF4B3C66,
              0xFF50FA7B,
              0xFF8BE9FD,
              0xFFFF5555,
              0xFF6272A4,
            )
          : _terminal(
              0xFFF8F8F2,
              0xFFFFFFFF,
              0xFFE8E8EE,
              0xFF282A36,
              0xFF7C4DCC,
              0xFFE2D5F5,
              0xFF087E3B,
              0xFF087C91,
              0xFFC62828,
              0xFF9A9AAF,
            ),
    TorcaThemeVariant.terminalSolarized =>
      dark
          ? _terminal(
              0xFF002B36,
              0xFF073642,
              0xFF0B3D49,
              0xFFEEE8D5,
              0xFF268BD2,
              0xFF164F67,
              0xFF859900,
              0xFF2AA198,
              0xFFDC322F,
              0xFF586E75,
            )
          : _terminal(
              0xFFFDF6E3,
              0xFFEEE8D5,
              0xFFE5DDC8,
              0xFF073642,
              0xFF268BD2,
              0xFFD5E7EF,
              0xFF859900,
              0xFF2AA198,
              0xFFDC322F,
              0xFF93A1A1,
            ),
  };
}

_Palette _modern(bool dark, Color primary, Color accent) => _Palette(
  background: dark ? const Color(0xFF0E151A) : const Color(0xFFF4F7F9),
  surface: dark ? const Color(0xFF151E24) : Colors.white,
  container: dark ? const Color(0xFF202B32) : const Color(0xFFEDF2F5),
  onSurface: dark ? const Color(0xFFE7EEF2) : const Color(0xFF182229),
  primary: primary,
  primaryContainer: dark
      ? primary.withValues(alpha: .28)
      : primary.withValues(alpha: .18),
  secondary: accent,
  accent: accent,
  error: dark ? const Color(0xFFFF6B6B) : const Color(0xFFB3261E),
  outline: dark ? const Color(0xFF40515B) : const Color(0xFFCBD5DB),
);

_Palette _terminal(
  int background,
  int surface,
  int container,
  int onSurface,
  int primary,
  int primaryContainer,
  int secondary,
  int accent,
  int error,
  int outline,
) => _Palette(
  background: Color(background),
  surface: Color(surface),
  container: Color(container),
  onSurface: Color(onSurface),
  primary: Color(primary),
  primaryContainer: Color(primaryContainer),
  secondary: Color(secondary),
  accent: Color(accent),
  error: Color(error),
  outline: Color(outline),
);
