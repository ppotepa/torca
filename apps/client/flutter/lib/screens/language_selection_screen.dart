import 'package:flutter/material.dart';

import '../localization/app_locale_mode.dart';
import '../localization/torca_strings.dart';
import '../settings/local_preferences.dart';

class LanguageSelectionScreen extends StatefulWidget {
  const LanguageSelectionScreen({required this.preferences, super.key});

  final LocalPreferences preferences;

  @override
  State<LanguageSelectionScreen> createState() =>
      _LanguageSelectionScreenState();
}

class _LanguageSelectionScreenState extends State<LanguageSelectionScreen> {
  bool _saving = false;

  Future<void> _choose(AppLocaleMode language) async {
    if (_saving) return;
    setState(() => _saving = true);
    await widget.preferences.chooseInitialLanguage(language);
    if (mounted) setState(() => _saving = false);
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    body: SafeArea(
      child: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 520),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Icon(
                  Icons.translate_rounded,
                  size: 56,
                  color: Theme.of(context).colorScheme.primary,
                ),
                const SizedBox(height: 20),
                Text(
                  context.strings.chooseLanguage,
                  textAlign: TextAlign.center,
                  style: Theme.of(context).textTheme.headlineMedium,
                ),
                const SizedBox(height: 6),
                Text(
                  AppLocaleMode.values
                      .where((mode) => mode != AppLocaleMode.system)
                      .map((mode) => mode.selectionPrompt)
                      .join('  •  '),
                  textAlign: TextAlign.center,
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
                const SizedBox(height: 28),
                for (final mode in AppLocaleMode.values)
                  if (mode != AppLocaleMode.system) ...<Widget>[
                    _LanguageCard(
                      flag: mode.flag,
                      title: mode.nativeName,
                      semanticLabel: mode.nativeName,
                      enabled: !_saving,
                      onTap: () => _choose(mode),
                    ),
                    if (mode != AppLocaleMode.ukrainian)
                      const SizedBox(height: 12),
                  ],
              ],
            ),
          ),
        ),
      ),
    ),
  );
}

class _LanguageCard extends StatelessWidget {
  const _LanguageCard({
    required this.flag,
    required this.title,
    required this.semanticLabel,
    required this.enabled,
    required this.onTap,
  });

  final String flag;
  final String title;
  final String semanticLabel;
  final bool enabled;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Semantics(
    button: true,
    enabled: enabled,
    label: semanticLabel,
    child: Card(
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: enabled ? onTap : null,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 18),
          child: Row(
            children: <Widget>[
              Text(flag, style: const TextStyle(fontSize: 34)),
              const SizedBox(width: 18),
              Expanded(
                child: Text(
                  title,
                  style: Theme.of(context).textTheme.titleLarge,
                ),
              ),
              const Icon(Icons.chevron_right_rounded),
            ],
          ),
        ),
      ),
    ),
  );
}
