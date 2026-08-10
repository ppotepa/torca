import 'package:flutter/material.dart';

@immutable
class TorcaTokens extends ThemeExtension<TorcaTokens> {
  const TorcaTokens({
    required this.terminal,
    required this.compact,
    required this.radiusSmall,
    required this.radiusMedium,
    required this.radiusLarge,
    required this.spaceUnit,
    required this.listTileHeight,
    required this.borderWidth,
    required this.animationDuration,
  });

  final bool terminal;
  final bool compact;
  final double radiusSmall;
  final double radiusMedium;
  final double radiusLarge;
  final double spaceUnit;
  final double listTileHeight;
  final double borderWidth;
  final Duration animationDuration;

  @override
  TorcaTokens copyWith({
    bool? terminal,
    bool? compact,
    double? radiusSmall,
    double? radiusMedium,
    double? radiusLarge,
    double? spaceUnit,
    double? listTileHeight,
    double? borderWidth,
    Duration? animationDuration,
  }) => TorcaTokens(
    terminal: terminal ?? this.terminal,
    compact: compact ?? this.compact,
    radiusSmall: radiusSmall ?? this.radiusSmall,
    radiusMedium: radiusMedium ?? this.radiusMedium,
    radiusLarge: radiusLarge ?? this.radiusLarge,
    spaceUnit: spaceUnit ?? this.spaceUnit,
    listTileHeight: listTileHeight ?? this.listTileHeight,
    borderWidth: borderWidth ?? this.borderWidth,
    animationDuration: animationDuration ?? this.animationDuration,
  );

  @override
  TorcaTokens lerp(covariant TorcaTokens? other, double t) {
    if (other == null) return this;
    return TorcaTokens(
      terminal: t < .5 ? terminal : other.terminal,
      compact: t < .5 ? compact : other.compact,
      radiusSmall: lerpDouble(radiusSmall, other.radiusSmall, t),
      radiusMedium: lerpDouble(radiusMedium, other.radiusMedium, t),
      radiusLarge: lerpDouble(radiusLarge, other.radiusLarge, t),
      spaceUnit: lerpDouble(spaceUnit, other.spaceUnit, t),
      listTileHeight: lerpDouble(listTileHeight, other.listTileHeight, t),
      borderWidth: lerpDouble(borderWidth, other.borderWidth, t),
      animationDuration: t < .5 ? animationDuration : other.animationDuration,
    );
  }
}

double lerpDouble(double a, double b, double t) => a + (b - a) * t;

extension TorcaTokenContext on BuildContext {
  TorcaTokens get torcaTokens => Theme.of(this).extension<TorcaTokens>()!;
}
