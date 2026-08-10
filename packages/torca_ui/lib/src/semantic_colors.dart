import 'package:flutter/material.dart';

@immutable
class TorcaSemanticColors extends ThemeExtension<TorcaSemanticColors> {
  const TorcaSemanticColors({
    required this.connectionReady,
    required this.connectionConnecting,
    required this.connectionOffline,
    required this.warning,
    required this.destructive,
    required this.messageInbound,
    required this.messageOutbound,
    required this.unreadBadge,
    required this.chatBackground,
    required this.separator,
  });

  final Color connectionReady;
  final Color connectionConnecting;
  final Color connectionOffline;
  final Color warning;
  final Color destructive;
  final Color messageInbound;
  final Color messageOutbound;
  final Color unreadBadge;
  final Color chatBackground;
  final Color separator;

  factory TorcaSemanticColors.fromScheme(ColorScheme scheme) =>
      TorcaSemanticColors(
        connectionReady: scheme.primary,
        connectionConnecting: scheme.tertiary,
        connectionOffline: scheme.outline,
        warning: scheme.tertiary,
        destructive: scheme.error,
        messageInbound: scheme.surfaceContainerHighest,
        messageOutbound: scheme.primaryContainer,
        unreadBadge: scheme.primary,
        chatBackground: scheme.surface,
        separator: scheme.outlineVariant,
      );

  @override
  TorcaSemanticColors copyWith({
    Color? connectionReady,
    Color? connectionConnecting,
    Color? connectionOffline,
    Color? warning,
    Color? destructive,
    Color? messageInbound,
    Color? messageOutbound,
    Color? unreadBadge,
    Color? chatBackground,
    Color? separator,
  }) => TorcaSemanticColors(
    connectionReady: connectionReady ?? this.connectionReady,
    connectionConnecting: connectionConnecting ?? this.connectionConnecting,
    connectionOffline: connectionOffline ?? this.connectionOffline,
    warning: warning ?? this.warning,
    destructive: destructive ?? this.destructive,
    messageInbound: messageInbound ?? this.messageInbound,
    messageOutbound: messageOutbound ?? this.messageOutbound,
    unreadBadge: unreadBadge ?? this.unreadBadge,
    chatBackground: chatBackground ?? this.chatBackground,
    separator: separator ?? this.separator,
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
      warning: mix(warning, other.warning),
      destructive: mix(destructive, other.destructive),
      messageInbound: mix(messageInbound, other.messageInbound),
      messageOutbound: mix(messageOutbound, other.messageOutbound),
      unreadBadge: mix(unreadBadge, other.unreadBadge),
      chatBackground: mix(chatBackground, other.chatBackground),
      separator: mix(separator, other.separator),
    );
  }
}

extension TorcaSemanticContext on BuildContext {
  TorcaSemanticColors get torcaColors =>
      Theme.of(this).extension<TorcaSemanticColors>()!;
}
