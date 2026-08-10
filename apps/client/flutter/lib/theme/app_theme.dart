import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import 'app_theme_mode.dart';

abstract final class AppTheme {
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
    return TorcaThemeFactory.build(appearance, brightness);
  }
}
