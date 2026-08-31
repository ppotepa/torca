import 'package:flutter/widgets.dart';

enum AppLocaleMode {
  system,
  english,
  polish,
  german,
  spanish,
  french,
  ukrainian,
}

extension AppLocaleModeValue on AppLocaleMode {
  String get storageValue => switch (this) {
    AppLocaleMode.system => 'system',
    AppLocaleMode.english => 'en',
    AppLocaleMode.polish => 'pl',
    AppLocaleMode.german => 'de',
    AppLocaleMode.spanish => 'es',
    AppLocaleMode.french => 'fr',
    AppLocaleMode.ukrainian => 'uk',
  };

  Locale? get locale => switch (this) {
    AppLocaleMode.system => null,
    AppLocaleMode.english => const Locale('en'),
    AppLocaleMode.polish => const Locale('pl'),
    AppLocaleMode.german => const Locale('de'),
    AppLocaleMode.spanish => const Locale('es'),
    AppLocaleMode.french => const Locale('fr'),
    AppLocaleMode.ukrainian => const Locale('uk'),
  };

  String get nativeName => switch (this) {
    AppLocaleMode.system => 'System',
    AppLocaleMode.english => 'English',
    AppLocaleMode.polish => 'Polski',
    AppLocaleMode.german => 'Deutsch',
    AppLocaleMode.spanish => 'Español',
    AppLocaleMode.french => 'Français',
    AppLocaleMode.ukrainian => 'Українська',
  };

  String get flag => switch (this) {
    AppLocaleMode.system => '🌐',
    AppLocaleMode.english => '🇬🇧',
    AppLocaleMode.polish => '🇵🇱',
    AppLocaleMode.german => '🇩🇪',
    AppLocaleMode.spanish => '🇪🇸',
    AppLocaleMode.french => '🇫🇷',
    AppLocaleMode.ukrainian => '🇺🇦',
  };

  String get selectionPrompt => switch (this) {
    AppLocaleMode.system || AppLocaleMode.english => 'Choose your language',
    AppLocaleMode.polish => 'Wybierz język',
    AppLocaleMode.german => 'Sprache auswählen',
    AppLocaleMode.spanish => 'Elige tu idioma',
    AppLocaleMode.french => 'Choisissez votre langue',
    AppLocaleMode.ukrainian => 'Оберіть мову',
  };
}

AppLocaleMode parseAppLocaleMode(String? value) => switch (value) {
  'en' => AppLocaleMode.english,
  'pl' => AppLocaleMode.polish,
  'de' => AppLocaleMode.german,
  'es' => AppLocaleMode.spanish,
  'fr' => AppLocaleMode.french,
  'uk' => AppLocaleMode.ukrainian,
  _ => AppLocaleMode.system,
};
