import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/theme/app_semantic_colors.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/theme/app_theme_mode.dart';
import 'package:torca_ui/torca_ui.dart';

void main() {
  test('theme mode storage values are stable', () {
    expect(AppThemeMode.parse(null), AppThemeMode.system);
    expect(AppThemeMode.parse('system'), AppThemeMode.system);
    expect(AppThemeMode.parse('light'), AppThemeMode.light);
    expect(AppThemeMode.parse('dark'), AppThemeMode.dark);
    expect(AppThemeMode.parse('unknown'), AppThemeMode.system);
  });

  test('light and dark themes expose semantic presentation colors', () {
    final light = AppTheme.light();
    final dark = AppTheme.dark();

    expect(light.brightness, Brightness.light);
    expect(dark.brightness, Brightness.dark);
    expect(light.extension<AppSemanticColors>(), isNotNull);
    expect(dark.extension<AppSemanticColors>(), isNotNull);
    expect(AppTheme.materialMode(AppThemeMode.system), ThemeMode.system);
    expect(AppTheme.materialMode(AppThemeMode.light), ThemeMode.light);
    expect(AppTheme.materialMode(AppThemeMode.dark), ThemeMode.dark);
  });

  test('app theme exposes modern and terminal appearance variants', () {
    final modern = AppTheme.dark();
    final terminal = AppTheme.dark(
      const TorcaAppearance(
        family: TorcaThemeFamily.terminal,
        variant: TorcaThemeVariant.terminalDracula,
      ),
    );
    expect(modern.extension<TorcaTokens>()!.terminal, isFalse);
    expect(terminal.extension<TorcaTokens>()!.terminal, isTrue);
    expect(terminal.extension<TorcaIconSet>(), isNotNull);
  });
}
