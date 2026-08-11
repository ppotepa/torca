import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/runtime_network_status.dart';

void main() {
  Widget monitor(AppSnapshotDto snapshot) => MaterialApp(
    theme: AppTheme.light(),
    home: Scaffold(
      body: Align(
        alignment: Alignment.topRight,
        child: RuntimeNetworkStatus(
          key: const ValueKey<String>('runtime-monitor'),
          snapshot: snapshot,
        ),
      ),
    ),
  );

  String? ledLabel(WidgetTester tester, String key) => tester
      .widget<Semantics>(
        find.descendant(
          of: find.byKey(ValueKey<String>(key)),
          matching: find.byType(Semantics),
        ),
      )
      .properties
      .label;

  testWidgets('network monitor marks a snapshot stale when polling stops', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: const Scaffold(
          body: Align(
            alignment: Alignment.topRight,
            child: RuntimeNetworkStatus(snapshot: AppSnapshotDto()),
          ),
        ),
      ),
    );

    expect(find.bySemanticsLabel(RegExp('monitoring stale')), findsNothing);

    await tester.pump(const Duration(seconds: 4));

    expect(find.bySemanticsLabel(RegExp('monitoring stale')), findsOneWidget);
  });

  testWidgets('batched TX and RX observations are replayed independently', (
    tester,
  ) async {
    await tester.pumpWidget(monitor(const AppSnapshotDto()));
    await tester.pumpWidget(
      monitor(
        const AppSnapshotDto(
          transport: TransportStatusDto(
            relay: TransportIndicatorDto(
              state: 'healthy',
              txSequence: 1,
              rxSequence: 1,
            ),
          ),
        ),
      ),
    );

    expect(ledLabel(tester, 'Relay-tx-led'), 'TX activity');
    expect(ledLabel(tester, 'Relay-rx-led'), 'RX idle');

    await tester.pump(const Duration(milliseconds: 250));

    expect(ledLabel(tester, 'Relay-tx-led'), 'TX idle');
    expect(ledLabel(tester, 'Relay-rx-led'), 'RX activity');
  });
}
