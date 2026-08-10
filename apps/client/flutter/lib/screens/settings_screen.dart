// ignore_for_file: deprecated_member_use

import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/app_locale_mode.dart';
import '../localization/torca_strings.dart';
import '../platform/platform_capabilities.dart';
import '../settings/local_preferences.dart';
import '../theme/app_theme_mode.dart';
import '../widgets/runtime_network_status.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({required this.preferences, super.key});

  final LocalPreferences preferences;

  @override
  Widget build(BuildContext context) {
    final strings = context.strings;
    return Scaffold(
      appBar: RuntimeAppBar(title: Text(strings.settings)),
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
                    children: <Widget>[
                      Padding(
                        padding: const EdgeInsets.fromLTRB(16, 14, 16, 8),
                        child: SegmentedButton<TorcaThemeFamily>(
                          segments: const <ButtonSegment<TorcaThemeFamily>>[
                            ButtonSegment(
                              value: TorcaThemeFamily.modern,
                              icon: Icon(Icons.forum_outlined),
                              label: Text('Modern'),
                            ),
                            ButtonSegment(
                              value: TorcaThemeFamily.terminal,
                              icon: Icon(Icons.terminal),
                              label: Text('Terminal'),
                            ),
                          ],
                          selected: <TorcaThemeFamily>{
                            preferences.appearance.family,
                          },
                          onSelectionChanged: (value) =>
                              preferences.setThemeFamily(value.single),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 16),
                        child: DropdownButtonFormField<TorcaThemeVariant>(
                          key: ValueKey(preferences.appearance.family),
                          initialValue: preferences.appearance.variant,
                          decoration: const InputDecoration(
                            labelText: 'Variant',
                          ),
                          items: TorcaThemeVariant.values
                              .where(
                                (variant) =>
                                    variant.family ==
                                    preferences.appearance.family,
                              )
                              .map(
                                (variant) => DropdownMenuItem(
                                  value: variant,
                                  child: Text(variant.label),
                                ),
                              )
                              .toList(growable: false),
                          onChanged: (value) {
                            if (value != null) {
                              preferences.setThemeVariant(value);
                            }
                          },
                        ),
                      ),
                      const SizedBox(height: 12),
                      _AppearancePreview(appearance: preferences.appearance),
                      const Divider(height: 24),
                      ...AppThemeMode.values.map(
                        (mode) => RadioListTile<AppThemeMode>(
                          title: Text(_themeLabel(strings, mode)),
                          value: mode,
                          groupValue: preferences.themeMode,
                          onChanged: (value) {
                            if (value != null) {
                              preferences.setThemeMode(value);
                            }
                          },
                        ),
                      ),
                      const Divider(height: 1),
                      RadioListTile<TorcaDensity>(
                        title: const Text('Compact density'),
                        value: TorcaDensity.compact,
                        groupValue: preferences.appearance.density,
                        onChanged: (value) {
                          if (value != null) {
                            preferences.setThemeDensity(value);
                          }
                        },
                      ),
                      RadioListTile<TorcaDensity>(
                        title: const Text('Comfortable density'),
                        value: TorcaDensity.comfortable,
                        groupValue: preferences.appearance.density,
                        onChanged: (value) {
                          if (value != null) {
                            preferences.setThemeDensity(value);
                          }
                        },
                      ),
                      SwitchListTile(
                        secondary: const Icon(Icons.motion_photos_off_outlined),
                        title: const Text('Reduce motion'),
                        value: preferences.appearance.reduceMotion,
                        onChanged: preferences.setReduceMotion,
                      ),
                    ],
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

class _AppearancePreview extends StatelessWidget {
  const _AppearancePreview({required this.appearance});

  final TorcaAppearance appearance;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final preview = TorcaThemeFactory.build(appearance, brightness);
    return Theme(
      data: preview,
      child: Builder(
        builder: (context) => Container(
          margin: const EdgeInsets.symmetric(horizontal: 16),
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surface,
            border: Border.all(color: Theme.of(context).colorScheme.outline),
            borderRadius: BorderRadius.circular(
              context.torcaTokens.radiusMedium,
            ),
          ),
          child: Column(
            children: <Widget>[
              Row(
                children: <Widget>[
                  CircleAvatar(
                    radius: 18,
                    child: Icon(context.torcaIcons.contacts, size: 18),
                  ),
                  const SizedBox(width: 10),
                  const Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text('Private contact'),
                        Text('Secure message preview'),
                      ],
                    ),
                  ),
                  Badge.count(count: 2),
                ],
              ),
              const SizedBox(height: 10),
              Align(
                alignment: Alignment.centerRight,
                child: Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 8,
                  ),
                  decoration: BoxDecoration(
                    color: context.torcaColors.messageOutbound,
                    borderRadius: BorderRadius.circular(
                      context.torcaTokens.radiusMedium,
                    ),
                  ),
                  child: const Text('Hello through Tor'),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
