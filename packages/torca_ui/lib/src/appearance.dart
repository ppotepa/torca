import 'package:flutter/material.dart';

enum TorcaThemeFamily { modern, terminal }

enum TorcaThemeVariant {
  modernOcean,
  modernGraphite,
  modernForest,
  terminalGruvbox,
  terminalTokyoNight,
  terminalCatppuccin;

  TorcaThemeFamily get family => switch (this) {
    modernOcean || modernGraphite || modernForest => TorcaThemeFamily.modern,
    terminalGruvbox ||
    terminalTokyoNight ||
    terminalCatppuccin => TorcaThemeFamily.terminal,
  };

  String get label => switch (this) {
    modernOcean => 'Ocean',
    modernGraphite => 'Graphite',
    modernForest => 'Forest',
    terminalGruvbox => 'Gruvbox',
    terminalTokyoNight => 'Tokyo Night',
    terminalCatppuccin => 'Catppuccin',
  };

  static TorcaThemeVariant parse(String? value) {
    // Preserve preferences written before the terminal palette refresh.
    final migrated = switch (value) {
      'terminalDracula' => 'terminalTokyoNight',
      'terminalSolarized' => 'terminalCatppuccin',
      _ => value,
    };
    return values.firstWhere(
      (variant) => variant.name == migrated,
      orElse: () => TorcaThemeVariant.modernOcean,
    );
  }
}

enum TorcaDensity { compact, comfortable }

@immutable
class TorcaAppearance {
  const TorcaAppearance({
    this.family = TorcaThemeFamily.modern,
    this.variant = TorcaThemeVariant.modernOcean,
    this.density = TorcaDensity.compact,
    this.reduceMotion = false,
  });

  final TorcaThemeFamily family;
  final TorcaThemeVariant variant;
  final TorcaDensity density;
  final bool reduceMotion;

  TorcaAppearance copyWith({
    TorcaThemeFamily? family,
    TorcaThemeVariant? variant,
    TorcaDensity? density,
    bool? reduceMotion,
  }) {
    final nextFamily = family ?? this.family;
    final requested = variant ?? this.variant;
    final nextVariant = requested.family == nextFamily
        ? requested
        : TorcaThemeVariant.values.firstWhere(
            (value) => value.family == nextFamily,
          );
    return TorcaAppearance(
      family: nextFamily,
      variant: nextVariant,
      density: density ?? this.density,
      reduceMotion: reduceMotion ?? this.reduceMotion,
    );
  }

  static TorcaThemeFamily parseFamily(String? value) => switch (value) {
    'terminal' => TorcaThemeFamily.terminal,
    _ => TorcaThemeFamily.modern,
  };

  static TorcaDensity parseDensity(String? value) => switch (value) {
    'comfortable' => TorcaDensity.comfortable,
    _ => TorcaDensity.compact,
  };

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is TorcaAppearance &&
          family == other.family &&
          variant == other.variant &&
          density == other.density &&
          reduceMotion == other.reduceMotion;

  @override
  int get hashCode => Object.hash(family, variant, density, reduceMotion);
}
