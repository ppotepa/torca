import 'package:flutter/material.dart';

@immutable
class TorcaSemanticColors extends ThemeExtension<TorcaSemanticColors> {
  const TorcaSemanticColors({
    required this.connectionReady,
    required this.connectionConnecting,
    required this.connectionOffline,
    required this.success,
    required this.warning,
    required this.error,
    required this.destructive,
    required this.messageInbound,
    required this.messageOutbound,
    required this.unreadBadge,
    required this.chatBackground,
    required this.separator,
    required this.activityTransmit,
    required this.activityReceive,
    required this.activityIdle,
    required this.inactiveIndicator,
  });

  final Color connectionReady;
  final Color connectionConnecting;
  final Color connectionOffline;
  final Color success;
  final Color warning;
  final Color error;
  final Color destructive;
  final Color messageInbound;
  final Color messageOutbound;
  final Color unreadBadge;
  final Color chatBackground;
  final Color separator;
  final Color activityTransmit;
  final Color activityReceive;
  final Color activityIdle;
  final Color inactiveIndicator;

  factory TorcaSemanticColors.fromScheme(ColorScheme scheme) =>
      TorcaSemanticColors(
        connectionReady: scheme.primary,
        connectionConnecting: scheme.tertiary,
        connectionOffline: scheme.outline,
        success: scheme.primary,
        warning: scheme.tertiary,
        error: scheme.error,
        destructive: scheme.error,
        messageInbound: scheme.surfaceContainerHighest,
        messageOutbound: scheme.primaryContainer,
        unreadBadge: scheme.primary,
        chatBackground: scheme.surface,
        separator: scheme.outlineVariant,
        activityTransmit: scheme.primary,
        activityReceive: scheme.tertiary,
        activityIdle: scheme.surfaceContainerHighest,
        inactiveIndicator: scheme.surfaceContainerHighest,
      );

  @override
  TorcaSemanticColors copyWith({
    Color? connectionReady,
    Color? connectionConnecting,
    Color? connectionOffline,
    Color? success,
    Color? warning,
    Color? error,
    Color? destructive,
    Color? messageInbound,
    Color? messageOutbound,
    Color? unreadBadge,
    Color? chatBackground,
    Color? separator,
    Color? activityTransmit,
    Color? activityReceive,
    Color? activityIdle,
    Color? inactiveIndicator,
  }) => TorcaSemanticColors(
    connectionReady: connectionReady ?? this.connectionReady,
    connectionConnecting: connectionConnecting ?? this.connectionConnecting,
    connectionOffline: connectionOffline ?? this.connectionOffline,
    success: success ?? this.success,
    warning: warning ?? this.warning,
    error: error ?? this.error,
    destructive: destructive ?? this.destructive,
    messageInbound: messageInbound ?? this.messageInbound,
    messageOutbound: messageOutbound ?? this.messageOutbound,
    unreadBadge: unreadBadge ?? this.unreadBadge,
    chatBackground: chatBackground ?? this.chatBackground,
    separator: separator ?? this.separator,
    activityTransmit: activityTransmit ?? this.activityTransmit,
    activityReceive: activityReceive ?? this.activityReceive,
    activityIdle: activityIdle ?? this.activityIdle,
    inactiveIndicator: inactiveIndicator ?? this.inactiveIndicator,
  );

  @override
  TorcaSemanticColors lerp(covariant TorcaSemanticColors? other, double t) {
    if (other == null) return this;
    Color mix(Color a, Color b) => Color.lerp(a, b, t)!;
    return TorcaSemanticColors(
      connectionReady: mix(connectionReady, other.connectionReady),
      connectionConnecting: mix(
        connectionConnecting,
        other.connectionConnecting,
      ),
      connectionOffline: mix(connectionOffline, other.connectionOffline),
      success: mix(success, other.success),
      warning: mix(warning, other.warning),
      error: mix(error, other.error),
      destructive: mix(destructive, other.destructive),
      messageInbound: mix(messageInbound, other.messageInbound),
      messageOutbound: mix(messageOutbound, other.messageOutbound),
      unreadBadge: mix(unreadBadge, other.unreadBadge),
      chatBackground: mix(chatBackground, other.chatBackground),
      separator: mix(separator, other.separator),
      activityTransmit: mix(activityTransmit, other.activityTransmit),
      activityReceive: mix(activityReceive, other.activityReceive),
      activityIdle: mix(activityIdle, other.activityIdle),
      inactiveIndicator: mix(inactiveIndicator, other.inactiveIndicator),
    );
  }
}

extension TorcaSemanticContext on BuildContext {
  TorcaSemanticColors get torcaColors =>
      Theme.of(this).extension<TorcaSemanticColors>()!;
}
