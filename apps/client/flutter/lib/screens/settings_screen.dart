import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/app_locale_mode.dart';
import '../localization/torca_strings.dart';
import '../platform/platform_capabilities.dart';
import '../settings/battery_preferences.dart';
import '../settings/local_preferences.dart';
import '../theme/app_theme.dart';
import '../theme/app_theme_mode.dart';
import '../widgets/runtime_network_status.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({
    required this.preferences,
    required this.gateway,
    super.key,
  });

  final LocalPreferences preferences;
  final EngineGateway gateway;

  @override
  Widget build(BuildContext context) {
    final strings = context.strings;
    return Scaffold(
      appBar: RuntimeAppBar(title: Text(strings.settings)),
      body: ListenableBuilder(
        listenable: preferences,
        builder: (context, _) {
          final strings = context.strings;
          // Settings contains several relatively expensive controls and a
          // themed preview. A sliver-backed list only lays out the visible
          // portion on mobile instead of rebuilding and laying out the entire
          // page synchronously whenever one preference changes.
          return ListView(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
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
                        segments: <ButtonSegment<TorcaThemeFamily>>[
                          ButtonSegment(
                            value: TorcaThemeFamily.modern,
                            icon: Icon(context.torcaIcons.chats),
                            label: Text(strings.modern),
                          ),
                          ButtonSegment(
                            value: TorcaThemeFamily.terminal,
                            icon: Icon(context.torcaIcons.diagnostics),
                            label: Text(strings.terminal),
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
                        decoration: const InputDecoration(labelText: 'Variant'),
                        icon: Icon(context.torcaIcons.expand),
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
                      (mode) => TorcaRadioTile<AppThemeMode>(
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
                    TorcaRadioTile<TorcaDensity>(
                      title: Text(strings.compactDensity),
                      value: TorcaDensity.compact,
                      groupValue: preferences.appearance.density,
                      onChanged: (value) {
                        if (value != null) {
                          preferences.setThemeDensity(value);
                        }
                      },
                    ),
                    TorcaRadioTile<TorcaDensity>(
                      title: Text(strings.comfortableDensity),
                      value: TorcaDensity.comfortable,
                      groupValue: preferences.appearance.density,
                      onChanged: (value) {
                        if (value != null) {
                          preferences.setThemeDensity(value);
                        }
                      },
                    ),
                    TorcaSwitchTile(
                      secondary: Icon(context.torcaIcons.warning),
                      title: Text(strings.reduceMotion),
                      value: preferences.appearance.reduceMotion,
                      onChanged: preferences.setReduceMotion,
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 24),
              Text(
                'Battery & availability',
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: 8),
              _BatterySettingsCard(preferences: preferences),
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
                        (mode) => TorcaRadioTile<AppLocaleMode>(
                          title: Text(_localeLabel(strings, mode)),
                          value: mode,
                          groupValue: preferences.localeMode,
                          onChanged: (value) {
                            if (value != null) preferences.setLocaleMode(value);
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
                child: TorcaSwitchTile(
                  secondary: Icon(context.torcaIcons.read),
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
                child: TorcaSwitchTile(
                  secondary: Icon(context.torcaIcons.notifications),
                  title: Text(strings.enableNotifications),
                  subtitle: Text(strings.notificationPrivacy),
                  value: preferences.notificationsEnabled,
                  onChanged: preferences.setNotificationsEnabled,
                ),
              ),
              if (isTorcaWindows) ...<Widget>[
                const SizedBox(height: 24),
                Text(
                  strings.audio,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                _AudioSettingsCard(gateway: gateway, preferences: preferences),
                const SizedBox(height: 24),
                Text(
                  strings.desktop,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                Card(
                  child: TorcaSwitchTile(
                    secondary: Icon(context.torcaIcons.save),
                    title: Text(strings.closeToTray),
                    subtitle: Text(strings.closeToTrayDescription),
                    value: preferences.closeToTrayEnabled,
                    onChanged: preferences.setCloseToTrayEnabled,
                  ),
                ),
              ],
            ],
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

class _BatterySettingsCard extends StatelessWidget {
  const _BatterySettingsCard({required this.preferences});

  final LocalPreferences preferences;

  @override
  Widget build(BuildContext context) => Card(
    child: Column(
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 14, 16, 4),
          child: Text(
            'Choose when Torca may defer background work. Incoming work is never silently discarded.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ),
        _select<TorcaBatteryMode>(
          context,
          label: 'Availability mode',
          value: preferences.batteryMode,
          values: TorcaBatteryMode.values,
          labelFor: _batteryModeLabel,
          onChanged: preferences.setBatteryMode,
        ),
        _select<TorcaBackgroundSyncCadence>(
          context,
          label: 'Background sync',
          value: preferences.backgroundSync,
          values: TorcaBackgroundSyncCadence.values,
          labelFor: _cadenceLabel,
          onChanged: preferences.setBackgroundSync,
        ),
        TorcaSwitchTile(
          secondary: Icon(context.torcaIcons.save),
          title: const Text('Allow delayed background delivery'),
          subtitle: const Text(
            'Required before Automatic or Saver can suspend Tor while the app is idle.',
          ),
          value: preferences.allowDelayedBackgroundDelivery,
          onChanged: preferences.setAllowDelayedBackgroundDelivery,
        ),
        _select<TorcaMeteredTransferPolicy>(
          context,
          label: 'Metered network transfers',
          value: preferences.meteredTransfers,
          values: TorcaMeteredTransferPolicy.values,
          labelFor: _meteredLabel,
          onChanged: preferences.setMeteredTransfers,
        ),
        _select<TorcaVisualActivityPolicy>(
          context,
          label: 'Avatar and visual activity',
          value: preferences.visualActivity,
          values: TorcaVisualActivityPolicy.values,
          labelFor: _visualLabel,
          onChanged: preferences.setVisualActivity,
        ),
        const SizedBox(height: 8),
      ],
    ),
  );

  Widget _select<T>(
    BuildContext context, {
    required String label,
    required T value,
    required List<T> values,
    required String Function(T) labelFor,
    required Future<void> Function(T) onChanged,
  }) => Padding(
    padding: const EdgeInsets.fromLTRB(16, 8, 16, 4),
    child: DropdownButtonFormField<T>(
      initialValue: value,
      decoration: InputDecoration(labelText: label),
      items: values
          .map(
            (item) =>
                DropdownMenuItem<T>(value: item, child: Text(labelFor(item))),
          )
          .toList(growable: false),
      onChanged: (next) {
        if (next != null) onChanged(next);
      },
    ),
  );

  String _batteryModeLabel(TorcaBatteryMode value) => switch (value) {
    TorcaBatteryMode.automatic => 'Automatic',
    TorcaBatteryMode.alwaysAvailable => 'Always available',
    TorcaBatteryMode.balanced => 'Balanced',
    TorcaBatteryMode.batterySaver => 'Battery saver',
  };

  String _cadenceLabel(TorcaBackgroundSyncCadence value) => switch (value) {
    TorcaBackgroundSyncCadence.instant => 'Instant',
    TorcaBackgroundSyncCadence.fiveMinutes => 'Every 5 minutes',
    TorcaBackgroundSyncCadence.fifteenMinutes => 'Every 15 minutes',
    TorcaBackgroundSyncCadence.thirtyMinutes => 'Every 30 minutes',
    TorcaBackgroundSyncCadence.hourly => 'Every hour',
    TorcaBackgroundSyncCadence.twoHours => 'Every 2 hours',
    TorcaBackgroundSyncCadence.onOpen => 'Only when opened',
  };

  String _meteredLabel(TorcaMeteredTransferPolicy value) => switch (value) {
    TorcaMeteredTransferPolicy.allowAll => 'Allow all',
    TorcaMeteredTransferPolicy.pauseLarge => 'Pause large files',
    TorcaMeteredTransferPolicy.pauseAll => 'Pause all transfers',
  };

  String _visualLabel(TorcaVisualActivityPolicy value) => switch (value) {
    TorcaVisualActivityPolicy.full => 'Full animation',
    TorcaVisualActivityPolicy.focusedOnly => 'Animate focused views',
    TorcaVisualActivityPolicy.staticOnly => 'Static when idle',
    TorcaVisualActivityPolicy.followSystem => 'Follow system setting',
  };
}

const _systemDefaultDeviceId = '__system_default__';

class _AudioSettingsCard extends StatelessWidget {
  const _AudioSettingsCard({required this.gateway, required this.preferences});

  final EngineGateway gateway;
  final LocalPreferences preferences;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: gateway.snapshots,
    builder: (context, snapshot, _) {
      final audio = snapshot.radio.audio;
      return Card(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            children: <Widget>[
              _deviceDropdown(
                context,
                label: context.strings.microphone,
                devices: audio.inputDevices,
                selectedId: preferences.audioInputDeviceId,
                onChanged: preferences.setAudioInputDevice,
              ),
              const SizedBox(height: 12),
              _deviceDropdown(
                context,
                label: context.strings.audioOutput,
                devices: audio.outputDevices,
                selectedId: preferences.audioOutputDeviceId,
                onChanged: preferences.setAudioOutputDevice,
              ),
            ],
          ),
        ),
      );
    },
  );

  Widget _deviceDropdown(
    BuildContext context, {
    required String label,
    required List<AudioDeviceDto> devices,
    required String? selectedId,
    required Future<void> Function(String? value) onChanged,
  }) {
    final availableSelection = devices.any((device) => device.id == selectedId)
        ? selectedId
        : _systemDefaultDeviceId;
    return DropdownButtonFormField<String>(
      key: ValueKey('$label:$availableSelection:${devices.length}'),
      initialValue: availableSelection,
      decoration: InputDecoration(labelText: label),
      items: <DropdownMenuItem<String>>[
        DropdownMenuItem<String>(
          value: _systemDefaultDeviceId,
          child: Text(context.strings.systemDefaultAudioDevice),
        ),
        ...devices.map(
          (device) => DropdownMenuItem<String>(
            value: device.id,
            child: Text(
              device.isDefault
                  ? context.strings.defaultAudioDevice(device.name)
                  : device.name,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ),
      ],
      onChanged: (value) async {
        try {
          await onChanged(value == _systemDefaultDeviceId ? null : value);
        } on Object {
          if (!context.mounted) return;
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(context.strings.audioDeviceUnavailable)),
          );
        }
      },
    );
  }
}

class _AppearancePreview extends StatelessWidget {
  const _AppearancePreview({required this.appearance});

  final TorcaAppearance appearance;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    // Reuse the process theme cache. Building a complete ThemeData here used
    // to duplicate MaterialApp's work and produced >500 ms UI stalls on the
    // Android settings route.
    final preview = brightness == Brightness.dark
        ? AppTheme.dark(appearance)
        : AppTheme.light(appearance);
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
                  Container(
                    width: 38,
                    height: 38,
                    decoration: BoxDecoration(
                      color: Theme.of(context).colorScheme.primaryContainer,
                      border: Border.all(
                        color: Theme.of(context).colorScheme.outline,
                      ),
                      borderRadius: BorderRadius.circular(
                        context.torcaTokens.terminal ? 0 : 19,
                      ),
                    ),
                    child: Icon(context.torcaIcons.contacts, size: 18),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(context.strings.sampleContactName),
                        Text(
                          context.strings.sampleOnline,
                          style: Theme.of(context).textTheme.bodySmall
                              ?.copyWith(
                                color: Theme.of(context).colorScheme.primary,
                              ),
                        ),
                      ],
                    ),
                  ),
                  Text(
                    context.strings.sampleTime,
                    style: Theme.of(context).textTheme.labelSmall,
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Text(
                context.strings.todayUpper,
                style: Theme.of(context).textTheme.labelSmall,
              ),
              const SizedBox(height: 8),
              const _PreviewMessage(
                body: 'Are we still meeting?',
                time: '14:20',
              ),
              const SizedBox(height: 5),
              const _PreviewMessage(
                body: 'Yes, give me 5 min.',
                time: '14:21  ✓✓',
                outbound: true,
              ),
              const SizedBox(height: 5),
              const _PreviewMessage(body: 'Perfect!', time: '14:22'),
              const SizedBox(height: 10),
              TextField(
                readOnly: true,
                decoration: InputDecoration(
                  hintText: 'Message',
                  prefixIcon: Icon(context.torcaIcons.attachment),
                  suffixIcon: Icon(context.torcaIcons.send),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PreviewMessage extends StatelessWidget {
  const _PreviewMessage({
    required this.body,
    required this.time,
    this.outbound = false,
  });

  final String body;
  final String time;
  final bool outbound;

  @override
  Widget build(BuildContext context) => Align(
    alignment: outbound ? Alignment.centerRight : Alignment.centerLeft,
    child: Container(
      constraints: const BoxConstraints(maxWidth: 250),
      padding: const EdgeInsets.fromLTRB(10, 7, 8, 5),
      decoration: BoxDecoration(
        color: outbound
            ? context.torcaColors.messageOutbound
            : context.torcaColors.messageInbound,
        border: context.torcaTokens.terminal
            ? Border.all(color: Theme.of(context).colorScheme.outline)
            : null,
        borderRadius: BorderRadius.circular(context.torcaTokens.radiusLarge),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: <Widget>[
          Text(body),
          const SizedBox(height: 3),
          Text(time, style: Theme.of(context).textTheme.labelSmall),
        ],
      ),
    ),
  );
}
