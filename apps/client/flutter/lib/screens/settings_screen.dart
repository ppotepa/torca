// ignore_for_file: deprecated_member_use

import 'package:flutter/material.dart';

import '../localization/app_locale_mode.dart';
import '../localization/torca_strings.dart';
import '../platform/platform_capabilities.dart';
import '../settings/local_preferences.dart';
import '../theme/app_theme_mode.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({required this.preferences, super.key});

  final LocalPreferences preferences;

  @override
  Widget build(BuildContext context) {
    final strings = context.strings;
    return Scaffold(
      appBar: AppBar(title: Text(strings.settings)),
      body: ListenableBuilder(
        listenable: preferences,
        builder: (context, _) {
          final strings = context.strings;
          return SingleChildScrollView(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
            child: Column(
              children: <Widget>[
                Text(
                  strings.appearance,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                Card(
                  child: Column(
                    children: AppThemeMode.values
                        .map(
                          (mode) => RadioListTile<AppThemeMode>(
                            title: Text(_themeLabel(strings, mode)),
                            value: mode,
                            groupValue: preferences.themeMode,
                            onChanged: (value) {
                              if (value != null)
                                preferences.setThemeMode(value);
                            },
                          ),
                        )
                        .toList(growable: false),
                  ),
                ),
                const SizedBox(height: 24),
                Text(
                  strings.language,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                Card(
                  child: Column(
                    children: AppLocaleMode.values
                        .map(
                          (mode) => RadioListTile<AppLocaleMode>(
                            title: Text(_localeLabel(strings, mode)),
                            value: mode,
                            groupValue: preferences.localeMode,
                            onChanged: (value) {
                              if (value != null)
                                preferences.setLocaleMode(value);
                            },
                          ),
                        )
                        .toList(growable: false),
                  ),
                ),
                const SizedBox(height: 24),
                Text(
                  strings.privacy,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                Card(
                  child: SwitchListTile(
                    secondary: const Icon(Icons.visibility_outlined),
                    title: Text(strings.sendReadReceipts),
                    subtitle: Text(strings.sendReadReceiptsDescription),
                    value: preferences.readReceiptsEnabled,
                    onChanged: preferences.setReadReceiptsEnabled,
                  ),
                ),
                const SizedBox(height: 24),
                Text(
                  strings.notifications,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                Card(
                  child: SwitchListTile(
                    secondary: const Icon(Icons.notifications_outlined),
                    title: Text(strings.enableNotifications),
                    subtitle: Text(strings.notificationPrivacy),
                    value: preferences.notificationsEnabled,
                    onChanged: preferences.setNotificationsEnabled,
                  ),
                ),
                if (isTorcaWindows) ...<Widget>[
                  const SizedBox(height: 24),
                  Text(
                    strings.desktop,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  Card(
                    child: SwitchListTile(
                      secondary: const Icon(Icons.move_to_inbox_outlined),
                      title: Text(strings.closeToTray),
                      subtitle: Text(strings.closeToTrayDescription),
                      value: preferences.closeToTrayEnabled,
                      onChanged: preferences.setCloseToTrayEnabled,
                    ),
                  ),
                ],
              ],
            ),
          );
        },
      ),
    );
  }

  String _themeLabel(TorcaStrings strings, AppThemeMode mode) => switch (mode) {
    AppThemeMode.system => strings.system,
    AppThemeMode.light => strings.light,
    AppThemeMode.dark => strings.dark,
  };

  String _localeLabel(TorcaStrings strings, AppLocaleMode mode) =>
      switch (mode) {
        AppLocaleMode.system => strings.languageSystem,
        AppLocaleMode.english => strings.languageEnglish,
        AppLocaleMode.polish => strings.languagePolish,
      };
}
