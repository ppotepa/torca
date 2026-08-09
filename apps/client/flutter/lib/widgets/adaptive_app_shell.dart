import 'package:flutter/material.dart';

/// Shared responsive navigation frame for every authenticated Torca surface.
class AdaptiveAppShell extends StatelessWidget {
  const AdaptiveAppShell({
    required this.title,
    required this.destinations,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.body,
    this.actions = const <Widget>[],
    this.floatingActionButton,
    super.key,
  });

  final String title;
  final List<NavigationDestination> destinations;
  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final Widget body;
  final List<Widget> actions;
  final Widget? floatingActionButton;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final mobile = constraints.maxWidth < 600;
      final railExtended = constraints.maxWidth >= 960;
      final rail = NavigationRail(
        extended: railExtended,
        minExtendedWidth: 190,
        selectedIndex: selectedIndex,
        onDestinationSelected: onDestinationSelected,
        labelType: railExtended ? null : NavigationRailLabelType.all,
        leading: Padding(
          padding: const EdgeInsets.only(top: 12, bottom: 18),
          child: railExtended
              ? const Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: <Widget>[
                    Icon(Icons.shield_outlined),
                    SizedBox(width: 10),
                    Text('Torca'),
                  ],
                )
              : const Icon(Icons.shield_outlined),
        ),
        destinations: destinations
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
        appBar: AppBar(title: Text(title), actions: actions),
        body: body,
        floatingActionButton: floatingActionButton,
        bottomNavigationBar: mobile
            ? NavigationBar(
                selectedIndex: selectedIndex,
                onDestinationSelected: onDestinationSelected,
                destinations: destinations,
              )
            : null,
      );
      if (mobile) return content;
      return Row(
        children: <Widget>[
          rail,
          const VerticalDivider(width: 1),
          Expanded(child: content),
        ],
      );
    },
  );
}
