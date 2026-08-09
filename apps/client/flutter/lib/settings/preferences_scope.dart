import 'package:flutter/widgets.dart';

import 'local_preferences.dart';

class PreferencesScope extends InheritedNotifier<LocalPreferences> {
  const PreferencesScope({
    required this.preferences,
    required super.child,
    super.key,
  }) : super(notifier: preferences);

  final LocalPreferences preferences;

  static LocalPreferences? maybeOf(BuildContext context) => context
      .dependOnInheritedWidgetOfExactType<PreferencesScope>()
      ?.preferences;

  static LocalPreferences of(BuildContext context) {
    final preferences = maybeOf(context);
    assert(
      preferences != null,
      'PreferencesScope is missing above this context.',
    );
    return preferences!;
  }
}
