import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:torca_ui/torca_ui.dart';

import '../localization/torca_strings.dart';

import 'runtime_network_status.dart';

/// Shared responsive navigation frame for every authenticated Torca surface.
class AdaptiveAppShell extends StatefulWidget {
  const AdaptiveAppShell({
    required this.title,
    required this.destinations,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.body,
    this.actions = const <Widget>[],
    this.floatingActionButton,
    this.buildLabel = 'dev',
    this.relayLabel = 'rel —',
    this.onBuildInfo,
    super.key,
  });

  final String title;
  final List<NavigationDestination> destinations;
  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final Widget body;
  final List<Widget> actions;
  final Widget? floatingActionButton;
  final String buildLabel;
  final String relayLabel;
  final VoidCallback? onBuildInfo;

  @override
  State<AdaptiveAppShell> createState() => _AdaptiveAppShellState();
}

class _AdaptiveAppShellState extends State<AdaptiveAppShell> {
  bool _railExpanded = true;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final mobile = constraints.maxWidth < 600;
      final canExtendRail = constraints.maxWidth >= 960;
      final railExtended = canExtendRail && _railExpanded;
      final rail = NavigationRail(
        extended: railExtended,
        minExtendedWidth: 190,
        selectedIndex: widget.selectedIndex,
        onDestinationSelected: widget.onDestinationSelected,
        labelType: railExtended ? null : NavigationRailLabelType.all,
        leading: Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 12),
          child: IconButton(
            tooltip: railExtended ? 'Collapse navigation' : 'Expand navigation',
            onPressed: canExtendRail
                ? () => setState(() => _railExpanded = !_railExpanded)
                : null,
            icon: Icon(
              railExtended
                  ? context.torcaIcons.collapse
                  : context.torcaIcons.more,
            ),
          ),
        ),
        trailing: Expanded(
          child: Align(
            alignment: Alignment.bottomCenter,
            child: Padding(
              padding: const EdgeInsets.only(bottom: 14),
              child: _BuildFooter(
                expanded: railExtended,
                buildLabel: widget.buildLabel,
                relayLabel: widget.relayLabel,
                onTap: widget.onBuildInfo,
              ),
            ),
          ),
        ),
        destinations: widget.destinations
            .map(
              (destination) => NavigationRailDestination(
                icon: destination.icon,
                selectedIcon: destination.selectedIcon,
                label: Text(destination.label),
              ),
            )
            .toList(growable: false),
      );
      final content = Scaffold(
        appBar: RuntimeAppBar(
          title: Text(widget.title),
          actions: widget.actions,
        ),
        body: widget.body,
        floatingActionButton: widget.floatingActionButton,
        bottomNavigationBar: mobile
            ? Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: InkWell(
                      onTap: widget.onBuildInfo,
                      child: Text(
                        'build ${widget.buildLabel}  ·  ${widget.relayLabel}',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context).textTheme.labelSmall,
                      ),
                    ),
                  ),
                  NavigationBar(
                    selectedIndex: widget.selectedIndex,
                    onDestinationSelected: widget.onDestinationSelected,
                    destinations: widget.destinations,
                  ),
                ],
              )
            : null,
      );
      final shell = mobile
          ? content
          : Row(
              children: <Widget>[
                rail,
                const VerticalDivider(width: 1),
                Expanded(child: content),
              ],
            );
      return CallbackShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          const SingleActivator(LogicalKeyboardKey.digit1, control: true): () =>
              widget.onDestinationSelected(0),
          const SingleActivator(LogicalKeyboardKey.digit2, control: true): () =>
              widget.onDestinationSelected(1),
          const SingleActivator(LogicalKeyboardKey.digit3, control: true): () =>
              widget.onDestinationSelected(2),
          const SingleActivator(LogicalKeyboardKey.escape): () =>
              FocusManager.instance.primaryFocus?.unfocus(),
        },
        child: shell,
      );
    },
  );
}

class _BuildFooter extends StatelessWidget {
  const _BuildFooter({
    required this.expanded,
    required this.buildLabel,
    required this.relayLabel,
    this.onTap,
  });

  final bool expanded;
  final String buildLabel;
  final String relayLabel;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) => Tooltip(
    message: context.strings.buildTooltip(buildLabel, relayLabel),
    child: InkWell(
      onTap: onTap,
      child: expanded
          ? Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Icon(context.torcaIcons.identity, size: 22),
                const SizedBox(height: 5),
                const Text('Torca'),
                Text(
                  context.strings.buildLabel(buildLabel),
                  style: Theme.of(context).textTheme.labelSmall,
                ),
                Text(relayLabel, style: Theme.of(context).textTheme.labelSmall),
              ],
            )
          : Icon(context.torcaIcons.identity, size: 22),
    ),
  );
}
