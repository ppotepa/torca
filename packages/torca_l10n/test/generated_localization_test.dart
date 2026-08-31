import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_l10n/torca_l10n.dart';

void main() {
  testWidgets('generated catalogs load every supported locale', (tester) async {
    for (final locale in TorcaLocaleRegistry.locales) {
      await tester.pumpWidget(
        Localizations(
          locale: locale,
          delegates: const <LocalizationsDelegate<dynamic>>[
            TorcaLocalizations.delegate,
            GlobalWidgetsLocalizations.delegate,
          ],
          child: Builder(
            builder: (context) {
              final l10n = TorcaLocalizations.of(context)!;
              expect(l10n.settingsTitle, isNotEmpty);
              expect(l10n.chooseNickname, isNotEmpty);
              expect(l10n.messageSenderYou, isNotEmpty);
              return const SizedBox.shrink();
            },
          ),
        ),
      );
    }
  });
}
