import 'package:flutter/material.dart';

@immutable
class AppSemanticColors extends ThemeExtension<AppSemanticColors> {
  const AppSemanticColors({
    required this.connectionReady,
    required this.connectionConnecting,
    required this.connectionOffline,
    required this.warning,
    required this.destructive,
    required this.messageInbound,
    required this.messageOutbound,
  });

  final Color connectionReady;
  final Color connectionConnecting;
  final Color connectionOffline;
  final Color warning;
  final Color destructive;
  final Color messageInbound;
  final Color messageOutbound;

  factory AppSemanticColors.fromScheme(ColorScheme scheme) => AppSemanticColors(
        connectionReady: scheme.primary,
        connectionConnecting: scheme.tertiary,
        connectionOffline: scheme.outline,
        warning: scheme.tertiary,
        destructive: scheme.error,
        messageInbound: scheme.surfaceContainerHighest,
        messageOutbound: scheme.primaryContainer,
      );

  @override
  AppSemanticColors copyWith({
    Color? connectionReady,
    Color? connectionConnecting,
    Color? connectionOffline,
    Color? warning,
    Color? destructive,
    Color? messageInbound,
    Color? messageOutbound,
  }) =>
      AppSemanticColors(
        connectionReady: connectionReady ?? this.connectionReady,
        connectionConnecting: connectionConnecting ?? this.connectionConnecting,
        connectionOffline: connectionOffline ?? this.connectionOffline,
        warning: warning ?? this.warning,
        destructive: destructive ?? this.destructive,
        messageInbound: messageInbound ?? this.messageInbound,
        messageOutbound: messageOutbound ?? this.messageOutbound,
      );

  @override
  AppSemanticColors lerp(covariant AppSemanticColors? other, double t) {
    if (other == null) return this;
    return AppSemanticColors(
      connectionReady: Color.lerp(connectionReady, other.connectionReady, t)!,
      connectionConnecting:
          Color.lerp(connectionConnecting, other.connectionConnecting, t)!,
      connectionOffline: Color.lerp(connectionOffline, other.connectionOffline, t)!,
      warning: Color.lerp(warning, other.warning, t)!,
      destructive: Color.lerp(destructive, other.destructive, t)!,
      messageInbound: Color.lerp(messageInbound, other.messageInbound, t)!,
      messageOutbound: Color.lerp(messageOutbound, other.messageOutbound, t)!,
    );
  }
}

extension AppSemanticTheme on BuildContext {
  AppSemanticColors get semanticColors =>
      Theme.of(this).extension<AppSemanticColors>()!;
}
