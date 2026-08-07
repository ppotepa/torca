import 'package:flutter/material.dart';

import '../settings/local_preferences.dart';
import '../theme/app_theme_mode.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({required this.preferences, super.key});

  final LocalPreferences preferences;

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Settings')),
        body: ListenableBuilder(
          listenable: preferences,
          builder: (context, _) => ListView(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
            children: <Widget>[
              Text('Appearance', style: Theme.of(context).textTheme.titleMedium),
              const SizedBox(height: 8),
              Card(
                child: Column(
                  children: AppThemeMode.values
                      .map(
                        (mode) => RadioListTile<AppThemeMode>(
                          title: Text(_themeLabel(mode)),
                          value: mode,
                          groupValue: preferences.themeMode,
                          onChanged: (value) {
                            if (value != null) preferences.setThemeMode(value);
                          },
                        ),
                      )
                      .toList(growable: false),
                ),
              ),
              const SizedBox(height: 24),
              Text('Notifications', style: Theme.of(context).textTheme.titleMedium),
              const SizedBox(height: 8),
              Card(
                child: SwitchListTile(
                  secondary: const Icon(Icons.notifications_outlined),
                  title: const Text('Enable notifications'),
                  subtitle: const Text(
                    'Show private-message notifications without message content.',
                  ),
                  value: preferences.notificationsEnabled,
                  onChanged: preferences.setNotificationsEnabled,
                ),
              ),
            ],
          ),
        ),
      );

  String _themeLabel(AppThemeMode mode) => switch (mode) {
        AppThemeMode.system => 'System',
        AppThemeMode.light => 'Light',
        AppThemeMode.dark => 'Dark',
      };
}
