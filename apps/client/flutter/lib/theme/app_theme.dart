import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import 'app_theme_mode.dart';

abstract final class AppTheme {
  static final Map<String, ThemeData> _cache = <String, ThemeData>{};

  static ThemeData light([
    TorcaAppearance appearance = const TorcaAppearance(),
  ]) => _build(appearance, Brightness.light);
  static ThemeData dark([
    TorcaAppearance appearance = const TorcaAppearance(),
  ]) => _build(appearance, Brightness.dark);

  static ThemeMode materialMode(AppThemeMode mode) => switch (mode) {
    AppThemeMode.system => ThemeMode.system,
    AppThemeMode.light => ThemeMode.light,
    AppThemeMode.dark => ThemeMode.dark,
  };

  static ThemeData _build(TorcaAppearance appearance, Brightness brightness) {
    final key = [
      appearance.family.name,
      appearance.variant.name,
      appearance.density.name,
      appearance.reduceMotion,
      brightness.name,
    ].join('|');
    return _cache.putIfAbsent(
      key,
      () => TorcaThemeFactory.build(appearance, brightness),
    );
  }
}
