import 'package:flutter/widgets.dart';

import 'locale_definition.dart';

abstract final class TorcaLocaleRegistry {
  static const definitions = <TorcaLocaleDefinition>[
    TorcaLocaleDefinition(
      locale: Locale('en'),
      nativeName: 'English',
      flag: '🇬🇧',
    ),
    TorcaLocaleDefinition(
      locale: Locale('pl'),
      nativeName: 'Polski',
      flag: '🇵🇱',
    ),
    TorcaLocaleDefinition(
      locale: Locale('de'),
      nativeName: 'Deutsch',
      flag: '🇩🇪',
    ),
    TorcaLocaleDefinition(
      locale: Locale('es'),
      nativeName: 'Español',
      flag: '🇪🇸',
    ),
    TorcaLocaleDefinition(
      locale: Locale('fr'),
      nativeName: 'Français',
      flag: '🇫🇷',
    ),
    TorcaLocaleDefinition(
      locale: Locale('uk'),
      nativeName: 'Українська',
      flag: '🇺🇦',
    ),
  ];

  static const locales = <Locale>[
    Locale('en'),
    Locale('pl'),
    Locale('de'),
    Locale('es'),
    Locale('fr'),
    Locale('uk'),
  ];

  static TorcaLocaleDefinition? find(Locale locale) {
    for (final definition in definitions) {
      if (definition.locale.languageCode == locale.languageCode) {
        return definition;
      }
    }
    return null;
  }
}
