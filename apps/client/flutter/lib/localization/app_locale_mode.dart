import 'package:flutter/widgets.dart';

enum AppLocaleMode { system, english, polish }

extension AppLocaleModeValue on AppLocaleMode {
  String get storageValue => switch (this) {
        AppLocaleMode.system => 'system',
        AppLocaleMode.english => 'en',
        AppLocaleMode.polish => 'pl',
      };

  Locale? get locale => switch (this) {
        AppLocaleMode.system => null,
        AppLocaleMode.english => const Locale('en'),
        AppLocaleMode.polish => const Locale('pl'),
      };
}

AppLocaleMode parseAppLocaleMode(String? value) => switch (value) {
      'en' => AppLocaleMode.english,
      'pl' => AppLocaleMode.polish,
      _ => AppLocaleMode.system,
    };
