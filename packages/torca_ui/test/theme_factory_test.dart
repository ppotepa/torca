import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_ui/torca_ui.dart';

void main() {
  test(
    'every variant builds light and dark themes with required extensions',
    () {
      for (final variant in TorcaThemeVariant.values) {
        final appearance = TorcaAppearance(
          family: variant.family,
          variant: variant,
        );
        for (final brightness in Brightness.values) {
          final theme = TorcaThemeFactory.build(appearance, brightness);
          expect(theme.brightness, brightness);
          expect(theme.extension<TorcaTokens>(), isNotNull);
          expect(theme.extension<TorcaSemanticColors>(), isNotNull);
          expect(theme.extension<TorcaIconSet>(), isNotNull);
        }
      }
    },
  );

  test('terminal geometry and icons differ from modern presentation', () {
    final modern = TorcaThemeFactory.build(
      const TorcaAppearance(),
      Brightness.dark,
    );
    final terminal = TorcaThemeFactory.build(
      const TorcaAppearance(
        family: TorcaThemeFamily.terminal,
        variant: TorcaThemeVariant.terminalGruvbox,
      ),
      Brightness.dark,
    );
    final modernTokens = modern.extension<TorcaTokens>()!;
    final terminalTokens = terminal.extension<TorcaTokens>()!;
    expect(modernTokens.terminal, isFalse);
    expect(terminalTokens.terminal, isTrue);
    expect(terminalTokens.radiusMedium, lessThan(modernTokens.radiusMedium));
    expect(
      modern.iconButtonTheme.style?.shape?.resolve(<WidgetState>{}),
      isA<CircleBorder>(),
    );
    expect(
      modern.iconButtonTheme.style?.fixedSize?.resolve(<WidgetState>{}),
      const Size.square(48),
    );
    final terminalIconShape = terminal.iconButtonTheme.style?.shape?.resolve(
      <WidgetState>{},
    );
    expect(terminalIconShape, isA<RoundedRectangleBorder>());
    expect(
      (terminalIconShape! as RoundedRectangleBorder).borderRadius,
      BorderRadius.zero,
    );
    expect(
      terminal.extension<TorcaIconSet>()!.chats,
      isNot(modern.extension<TorcaIconSet>()!.chats),
    );
  });

  test('changing family selects a valid default variant', () {
    final terminal = const TorcaAppearance().copyWith(
      family: TorcaThemeFamily.terminal,
    );
    expect(terminal.variant.family, TorcaThemeFamily.terminal);
  });
}
