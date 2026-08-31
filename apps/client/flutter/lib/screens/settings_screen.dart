import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/app_locale_mode.dart';
import '../platform/platform_capabilities.dart';
import '../settings/battery_preferences.dart';
import '../settings/local_preferences.dart';
import '../theme/app_theme.dart';
import '../theme/app_theme_mode.dart';
import '../widgets/message_bubble.dart';
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
    return Scaffold(
      appBar: RuntimeAppBar(title: Text(context.l10n.settingsTitle)),
      body: ListenableBuilder(
        listenable: preferences,
        builder: (context, _) {
          final strings = context.l10n;
          // Settings contains several relatively expensive controls and a
          // themed preview. A sliver-backed list only lays out the visible
          // portion on mobile instead of rebuilding and laying out the entire
          // page synchronously whenever one preference changes.
          return Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 760),
              child: ListView(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 32),
                children: <Widget>[
                  Text(
                    context.l10n.appearanceTitle,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  Card(
                    child: Column(
                      children: <Widget>[
                        LayoutBuilder(
                          builder: (context, constraints) {
                            final compact = constraints.maxWidth < 390;
                            return Padding(
                              padding: const EdgeInsets.fromLTRB(16, 14, 16, 8),
                              child: SegmentedButton<TorcaThemeFamily>(
                                segments: <ButtonSegment<TorcaThemeFamily>>[
                                  ButtonSegment(
                                    value: TorcaThemeFamily.modern,
                                    icon: compact
                                        ? null
                                        : Icon(context.torcaIcons.chats),
                                    label: Text(strings.modern),
                                  ),
                                  ButtonSegment(
                                    value: TorcaThemeFamily.terminal,
                                    icon: compact
                                        ? null
                                        : Icon(context.torcaIcons.diagnostics),
                                    label: Text(strings.terminal),
                                  ),
                                ],
                                selected: <TorcaThemeFamily>{
                                  preferences.appearance.family,
                                },
                                onSelectionChanged: (value) =>
                                    preferences.setThemeFamily(value.single),
                              ),
                            );
                          },
                        ),
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 16),
                          child: DropdownButtonFormField<TorcaThemeVariant>(
                            key: ValueKey(preferences.appearance.family),
                            initialValue: preferences.appearance.variant,
                            decoration: InputDecoration(
                              labelText: strings.variant,
                            ),
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
                    strings.batteryAvailability,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  _BatterySettingsCard(preferences: preferences),
                  const SizedBox(height: 24),
                  Text(
                    context.l10n.languageTitle,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  Card(
                    child: Column(
                      children: AppLocaleMode.values
                          .map(
                            (mode) => TorcaRadioTile<AppLocaleMode>(
                              title: Text(_localeLabel(context, mode)),
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
                    context.l10n.privacyTitle,
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
                    context.l10n.notificationsTitle,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  Card(
                    child: TorcaSwitchTile(
                      secondary: Icon(context.torcaIcons.notifications),
                      title: Text(context.l10n.enableNotifications),
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
                    _AudioSettingsCard(
                      gateway: gateway,
                      preferences: preferences,
                    ),
                    const SizedBox(height: 24),
                    Text(
                      strings.desktop,
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    const SizedBox(height: 8),
                    Card(
                      child: TorcaSwitchTile(
                        secondary: Icon(context.torcaIcons.save),
                        title: Text(context.l10n.closeToTray),
                        subtitle: Text(strings.closeToTrayDescription),
                        value: preferences.closeToTrayEnabled,
                        onChanged: preferences.setCloseToTrayEnabled,
                      ),
                    ),
                  ],
                ],
              ),
            ),
          );
        },
      ),
    );
  }

  String _themeLabel(TorcaLocalizations strings, AppThemeMode mode) => switch (mode) {
    AppThemeMode.system => strings.system,
    AppThemeMode.light => strings.light,
    AppThemeMode.dark => strings.dark,
  };

  String _localeLabel(BuildContext context, AppLocaleMode mode) =>
      mode == AppLocaleMode.system
      ? '${mode.flag} ${context.l10n.systemLanguage}'
      : '${mode.flag} ${mode.nativeName}';
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
            context.l10n.batterySettingsDescription,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ),
        _select<TorcaBatteryMode>(
          context,
          label: context.l10n.availabilityMode,
          value: preferences.batteryMode,
          values: TorcaBatteryMode.values,
          labelFor: (value) => _batteryModeLabel(context, value),
          onChanged: preferences.setBatteryMode,
        ),
        TorcaSwitchTile(
          secondary: Icon(context.torcaIcons.save),
          title: Text(context.l10n.allowDelayedBackgroundDelivery),
          subtitle: Text(
            context.l10n.allowDelayedBackgroundDeliveryDescription,
          ),
          value: preferences.allowDelayedBackgroundDelivery,
          onChanged: preferences.setAllowDelayedBackgroundDelivery,
        ),
        _select<TorcaMeteredTransferPolicy>(
          context,
          label: context.l10n.meteredTransfers,
          value: preferences.meteredTransfers,
          values: TorcaMeteredTransferPolicy.values,
          labelFor: (value) => _meteredLabel(context, value),
          onChanged: preferences.setMeteredTransfers,
        ),
        _select<TorcaVisualActivityPolicy>(
          context,
          label: context.l10n.visualActivity,
          value: preferences.visualActivity,
          values: TorcaVisualActivityPolicy.values,
          labelFor: (value) => _visualLabel(context, value),
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

  String _batteryModeLabel(BuildContext context, TorcaBatteryMode value) =>
      switch (value) {
        TorcaBatteryMode.automatic => context.l10n.automatic,
        TorcaBatteryMode.alwaysAvailable => context.l10n.alwaysAvailable,
        TorcaBatteryMode.batterySaver => context.l10n.batterySaver,
      };

  String _meteredLabel(
    BuildContext context,
    TorcaMeteredTransferPolicy value,
  ) => switch (value) {
    TorcaMeteredTransferPolicy.allowAll => context.l10n.allowAll,
    TorcaMeteredTransferPolicy.pauseLarge => context.l10n.pauseLarge,
    TorcaMeteredTransferPolicy.pauseAll => context.l10n.pauseAll,
  };

  String _visualLabel(BuildContext context, TorcaVisualActivityPolicy value) =>
      switch (value) {
        TorcaVisualActivityPolicy.full => context.l10n.fullAnimation,
        TorcaVisualActivityPolicy.focusedOnly => context.l10n.focusedOnly,
        TorcaVisualActivityPolicy.staticOnly => context.l10n.staticIdle,
        TorcaVisualActivityPolicy.followSystem => context.l10n.followSystem,
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
                label: context.l10n.microphone,
                devices: audio.inputDevices,
                selectedId: preferences.audioInputDeviceId,
                onChanged: preferences.setAudioInputDevice,
              ),
              const SizedBox(height: 12),
              _deviceDropdown(
                context,
                label: context.l10n.audioOutput,
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
          child: Text(context.l10n.systemDefaultAudioDevice),
        ),
        ...devices.map(
          (device) => DropdownMenuItem<String>(
            value: device.id,
            child: Text(
              device.isDefault
                  ? context.l10n.defaultAudioDevice(device.name)
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
            SnackBar(content: Text(context.l10n.audioDeviceUnavailable)),
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
                        Text(context.l10n.sampleContactName),
                        Text(
                          context.l10n.sampleOnline,
                          style: Theme.of(context).textTheme.bodySmall
                              ?.copyWith(
                                color: Theme.of(context).colorScheme.primary,
                              ),
                        ),
                      ],
                    ),
                  ),
                  Text(
                    context.l10n.sampleTime,
                    style: Theme.of(context).textTheme.labelSmall,
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Text(
                context.l10n.todayUpper,
                style: Theme.of(context).textTheme.labelSmall,
              ),
              const SizedBox(height: 8),
              MessageBubble(
                message: const MessageDto(
                  id: 'appearance-inbound-1',
                  conversationId: 'appearance',
                  body: 'Are we still meeting?',
                  direction: 'inbound',
                  status: 'delivered',
                  createdAtMs: 1725106800000,
                ),
                senderLabel: context.l10n.sampleContactName,
                senderColorKey: 'appearance-contact',
                compactBottom: true,
                onLongPress: () {},
              ),
              const _PreviewMessage(
                body: 'I will be there shortly.',
                showSender: false,
              ),
              const SizedBox(height: 8),
              const _PreviewMessage(
                body: 'Yes, give me 5 min.',
                outbound: true,
                compactBottom: true,
              ),
              const _PreviewMessage(
                body: 'Perfect!',
                outbound: true,
                showSender: false,
              ),
              const SizedBox(height: 10),
              TextField(
                readOnly: true,
                decoration: InputDecoration(
                  hintText: context.l10n.message,
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
    this.outbound = false,
    this.showSender = true,
    this.compactBottom = false,
  });

  final String body;
  final bool outbound;
  final bool showSender;
  final bool compactBottom;

  @override
  Widget build(BuildContext context) => MessageBubble(
    message: MessageDto(
      id: 'appearance-${body.hashCode}',
      conversationId: 'appearance',
      body: body,
      direction: outbound ? 'outbound' : 'inbound',
      status: 'delivered',
      createdAtMs: 1725106800000,
      sentAtMs: outbound ? 1725106800000 : null,
      deliveredAtMs: outbound ? 1725106860000 : null,
    ),
    senderLabel: outbound
        ? context.l10n.senderYou
        : context.l10n.sampleContactName,
    senderColorKey: outbound ? 'appearance-local' : 'appearance-contact',
    showSender: showSender,
    compactTop: !showSender,
    compactBottom: compactBottom,
    onLongPress: () {},
  );
}


