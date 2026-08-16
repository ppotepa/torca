import 'package:flutter/material.dart';

import 'tokens.dart';

class TorcaSwitch extends StatelessWidget {
  const TorcaSwitch({
    required this.value,
    required this.onChanged,
    this.semanticLabel,
    super.key,
  });

  final bool value;
  final ValueChanged<bool>? onChanged;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) {
    if (!context.torcaTokens.terminal) {
      return Switch.adaptive(value: value, onChanged: onChanged);
    }
    final colors = Theme.of(context).colorScheme;
    final enabled = onChanged != null;
    return Semantics(
      label: semanticLabel,
      toggled: value,
      enabled: enabled,
      button: true,
      child: Tooltip(
        message: semanticLabel ?? (value ? 'On' : 'Off'),
        child: InkWell(
          onTap: enabled ? () => onChanged!(!value) : null,
          borderRadius: BorderRadius.zero,
          child: ConstrainedBox(
            constraints: const BoxConstraints(minWidth: 48, minHeight: 48),
            child: Center(
              child: AnimatedContainer(
                duration: context.torcaTokens.animationDuration,
                width: 46,
                height: 26,
                padding: const EdgeInsets.all(3),
                decoration: BoxDecoration(
                  color: value ? colors.primary : colors.surface,
                  border: Border.all(
                    color: value ? colors.primary : colors.outline,
                    width: 2,
                  ),
                ),
                child: AnimatedAlign(
                  duration: context.torcaTokens.animationDuration,
                  alignment: value
                      ? Alignment.centerRight
                      : Alignment.centerLeft,
                  child: Container(
                    width: 16,
                    height: 16,
                    color: value ? colors.onPrimary : colors.onSurfaceVariant,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class TorcaBadge extends StatelessWidget {
  const TorcaBadge({required this.label, super.key});

  final Widget label;

  @override
  Widget build(BuildContext context) {
    if (!context.torcaTokens.terminal) return Badge(label: label);
    final colors = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: colors.primary,
        border: Border.all(color: colors.onPrimary),
      ),
      child: DefaultTextStyle(
        style: Theme.of(
          context,
        ).textTheme.labelSmall!.copyWith(color: colors.onPrimary),
        child: label,
      ),
    );
  }
}

class TorcaRadioTile<T> extends StatelessWidget {
  const TorcaRadioTile({
    required this.title,
    required this.value,
    required this.groupValue,
    required this.onChanged,
    this.subtitle,
    super.key,
  });

  final Widget title;
  final Widget? subtitle;
  final T value;
  final T? groupValue;
  final ValueChanged<T?>? onChanged;

  @override
  Widget build(BuildContext context) {
    if (!context.torcaTokens.terminal) {
      return IgnorePointer(
        ignoring: onChanged == null,
        child: RadioGroup<T>(
          groupValue: groupValue,
          onChanged: onChanged ?? (_) {},
          child: RadioListTile<T>(
            title: title,
            subtitle: subtitle,
            value: value,
          ),
        ),
      );
    }
    final selected = value == groupValue;
    return ListTile(
      enabled: onChanged != null,
      leading: Text(selected ? '[X]' : '[ ]'),
      title: title,
      subtitle: subtitle,
      selected: selected,
      onTap: onChanged == null ? null : () => onChanged!(value),
    );
  }
}

class TorcaSwitchTile extends StatelessWidget {
  const TorcaSwitchTile({
    required this.title,
    required this.value,
    required this.onChanged,
    this.subtitle,
    this.secondary,
    super.key,
  });

  final Widget title;
  final Widget? subtitle;
  final Widget? secondary;
  final bool value;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context) {
    if (!context.torcaTokens.terminal) {
      return SwitchListTile(
        secondary: secondary,
        title: title,
        subtitle: subtitle,
        value: value,
        onChanged: onChanged,
      );
    }
    final enabled = onChanged != null;
    return ListTile(
      enabled: enabled,
      leading: secondary,
      title: title,
      subtitle: subtitle,
      trailing: TorcaSwitch(value: value, onChanged: onChanged),
      onTap: enabled ? () => onChanged!(!value) : null,
    );
  }
}
