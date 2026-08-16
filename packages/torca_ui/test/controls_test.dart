import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_ui/torca_ui.dart';

void main() {
  testWidgets('switch follows modern theme and remains interactive', (
    tester,
  ) async {
    var value = false;
    await tester.pumpWidget(
      _SwitchHarness(
        appearance: const TorcaAppearance(),
        value: value,
        onChanged: (next) => value = next,
      ),
    );

    expect(find.byType(Switch), findsOneWidget);
    await tester.tap(find.byType(Switch));
    expect(value, isTrue);
  });

  testWidgets('switch follows terminal geometry and semantics', (tester) async {
    var value = false;
    await tester.pumpWidget(
      _SwitchHarness(
        appearance: const TorcaAppearance(
          family: TorcaThemeFamily.terminal,
          variant: TorcaThemeVariant.terminalGruvbox,
        ),
        value: value,
        onChanged: (next) => value = next,
      ),
    );

    expect(find.byType(Switch), findsNothing);
    expect(find.bySemanticsLabel('Radio mode'), findsOneWidget);
    await tester.tap(find.byType(InkWell));
    expect(value, isTrue);
  });

  testWidgets('code typography is deterministic across theme families', (
    tester,
  ) async {
    for (final appearance in <TorcaAppearance>[
      const TorcaAppearance(),
      const TorcaAppearance(
        family: TorcaThemeFamily.terminal,
        variant: TorcaThemeVariant.terminalGruvbox,
      ),
    ]) {
      late TextStyle style;
      await tester.pumpWidget(
        MaterialApp(
          theme: TorcaThemeFactory.build(appearance, Brightness.light),
          home: Builder(
            builder: (context) {
              style = context.torcaCodeStyle();
              return const SizedBox();
            },
          ),
        ),
      );
      expect(style.fontFamily, 'packages/torca_ui/JetBrainsMono');
    }
  });
}

class _SwitchHarness extends StatelessWidget {
  const _SwitchHarness({
    required this.appearance,
    required this.value,
    required this.onChanged,
  });

  final TorcaAppearance appearance;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: TorcaThemeFactory.build(appearance, Brightness.light),
    home: Material(
      child: Center(
        child: TorcaSwitch(
          value: value,
          semanticLabel: 'Radio mode',
          onChanged: onChanged,
        ),
      ),
    ),
  );
}
