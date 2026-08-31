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
    final radiusMedium = terminal ? 0.0 : 8.0;
    final radiusLarge = terminal ? 0.0 : 12.0;
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
    final bodyFamily = terminal ? 'JetBrainsMono' : null;
    final bodyPackage = terminal ? 'torca_ui' : null;
    final body = terminal
        ? base.textTheme.apply(fontFamily: bodyFamily, package: bodyPackage)
        : base.textTheme;
    final text = body.copyWith(
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
        elevation: 0,
        focusElevation: 0,
        hoverElevation: 0,
        highlightElevation: 0,
        disabledElevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(radiusMedium),
        ),
      ),
      // Material's IconButton defaults to a circular shape even when every
      // other terminal control uses hard pixel corners. Keep the geometry
      // coherent across conversation actions (attach, send and radio) while
      // preserving the familiar circle in modern themes.
      iconButtonTheme: IconButtonThemeData(
        style: ButtonStyle(
          fixedSize: const WidgetStatePropertyAll<Size>(Size.square(48)),
          padding: const WidgetStatePropertyAll<EdgeInsets>(EdgeInsets.zero),
          // Keep icon actions visually light. The hit target remains 48dp,
          // but profile/contact actions must not render as solid circles.
          backgroundColor: const WidgetStatePropertyAll<Color>(
            Colors.transparent,
          ),
          overlayColor: WidgetStatePropertyAll<Color>(
            scheme.primary.withValues(alpha: .12),
          ),
          shape: WidgetStatePropertyAll<OutlinedBorder>(
            terminal
                ? const RoundedRectangleBorder(borderRadius: BorderRadius.zero)
                : const CircleBorder(),
          ),
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(shape: shape).copyWith(
          animationDuration: Duration.zero,
          elevation: const WidgetStatePropertyAll<double>(0),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          shape: shape,
          side: BorderSide(color: scheme.outline),
        ).copyWith(animationDuration: Duration.zero),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          shape: shape,
        ).copyWith(animationDuration: Duration.zero),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(shape: shape).copyWith(
          animationDuration: Duration.zero,
          elevation: const WidgetStatePropertyAll<double>(0),
        ),
      ),
      segmentedButtonTheme: SegmentedButtonThemeData(
        style: ButtonStyle(
          shape: WidgetStatePropertyAll<OutlinedBorder>(shape),
          side: WidgetStatePropertyAll<BorderSide>(
            BorderSide(color: scheme.outline),
          ),
        ),
      ),
      checkboxTheme: CheckboxThemeData(
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(terminal ? 0 : 3),
        ),
        side: BorderSide(color: scheme.outline, width: terminal ? 2 : 1),
      ),
      radioTheme: RadioThemeData(
        visualDensity: compact ? VisualDensity.compact : VisualDensity.standard,
      ),
      switchTheme: SwitchThemeData(
        trackOutlineColor: WidgetStatePropertyAll<Color>(scheme.outline),
        trackOutlineWidth: WidgetStatePropertyAll<double>(terminal ? 2 : 1),
      ),
      chipTheme: base.chipTheme.copyWith(
        shape: shape,
        side: BorderSide(color: scheme.outline),
      ),
      popupMenuTheme: PopupMenuThemeData(
        color: palette.surface,
        elevation: terminal ? 0 : 4,
        shape: shape,
      ),
      pageTransitionsTheme: const PageTransitionsTheme(
        builders: <TargetPlatform, PageTransitionsBuilder>{
          TargetPlatform.android: _NoPageTransitionsBuilder(),
          TargetPlatform.iOS: _NoPageTransitionsBuilder(),
          TargetPlatform.linux: _NoPageTransitionsBuilder(),
          TargetPlatform.macOS: _NoPageTransitionsBuilder(),
          TargetPlatform.windows: _NoPageTransitionsBuilder(),
          TargetPlatform.fuchsia: _NoPageTransitionsBuilder(),
        },
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
          // Keep chat messages distinct from the surrounding surface in both
          // light and dark palettes. The old Material defaults could collapse
          // into the chat background, especially for terminal variants.
          messageInbound: Color.alphaBlend(
            palette.outline.withValues(alpha: .18),
            palette.surface,
          ),
          messageOutbound: Color.alphaBlend(
            palette.primary.withValues(alpha: .24),
            palette.surface,
          ),
          chatBackground: palette.background,
          separator: palette.outline,
        ),
        terminal ? TorcaIconSet.terminal : TorcaIconSet.modern,
      ],
    );
  }
}

class _NoPageTransitionsBuilder extends PageTransitionsBuilder {
  const _NoPageTransitionsBuilder();

  @override
  Widget buildTransitions<T>(
    PageRoute<T> route,
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
    Widget child,
  ) => child;
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
    TorcaThemeVariant.terminalTokyoNight =>
      dark
          ? _terminal(
              0xFF1A1B26,
              0xFF24283B,
              0xFF292E42,
              0xFFC0CAF5,
              0xFF7AA2F7,
              0xFF3D59A1,
              0xFF9ECE6A,
              0xFFBB9AF7,
              0xFFF7768E,
              0xFF565F89,
            )
          : _terminal(
              0xFFD5D6DB,
              0xFFE1E2E7,
              0xFFC8CAD2,
              0xFF343B58,
              0xFF34548A,
              0xFFA8B5D1,
              0xFF485E30,
              0xFF5A4A78,
              0xFF8C4351,
              0xFF9699A3,
            ),
    TorcaThemeVariant.terminalCatppuccin =>
      dark
          ? _terminal(
              0xFF1E1E2E,
              0xFF181825,
              0xFF313244,
              0xFFCDD6F4,
              0xFF89B4FA,
              0xFF45475A,
              0xFFA6E3A1,
              0xFFCBA6F7,
              0xFFF38BA8,
              0xFF585B70,
            )
          : _terminal(
              0xFFEFF1F5,
              0xFFE6E9EF,
              0xFFDCE0E8,
              0xFF4C4F69,
              0xFF1E66F5,
              0xFFCCD0DA,
              0xFF40A02B,
              0xFF8839EF,
              0xFFD20F39,
              0xFF9CA0B0,
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
