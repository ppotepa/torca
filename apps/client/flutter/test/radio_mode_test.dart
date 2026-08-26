import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/screens/contact_details_screen.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/conversation_header.dart';
import 'package:torca_app/widgets/radio_conversation_controls.dart';
import 'package:torca_app/widgets/radio_indicator.dart';
import 'package:torca_app/widgets/runtime_network_status.dart';

class _RadioGateway implements EngineGateway {
  _RadioGateway(AppSnapshotDto snapshot) : _snapshots = ValueNotifier(snapshot);

  final ValueNotifier<AppSnapshotDto> _snapshots;
  final List<BridgeCommandDto> commands = <BridgeCommandDto>[];

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Stream<RuntimeEventDto> get events => const Stream<RuntimeEventDto>.empty();

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    commands.add(command);
    if (command is BeginRadioTransmissionCommandDto) {
      _snapshots.value = AppSnapshotDto(
        radio: RadioDto(
          session: RadioSessionDto(
            contactId: command.contactIdHex,
            sessionId: 'test-session',
            state: 'transmitting',
            floor: 'local',
            burstElapsedMs: 0,
            maxBurstMs: 10000,
          ),
        ),
      );
    }
    return const BridgeResultDto(ok: true, kind: 'accepted');
  }

  @override
  Future<void> sendLifecycle(String event) async {}

  @override
  Future<String> diagnosticsJson() async => '{"events":[]}';

  @override
  Future<String> diagnosticsLogTailsJson() async => '{"logs":[]}';

  @override
  Future<void> dispose() async => _snapshots.dispose();
}

const _contactId = '00000000000000000000000000000001';
const _contact = ContactDto(
  id: _contactId,
  displayName: 'Alice',
  onionAddress: 'alice.onion',
  status: 'active',
  connectionState: 'ready',
);

void main() {
  testWidgets('Radio indicator exposes the ready state without starting work', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: const Scaffold(
          body: RadioIndicator(
            radio: RadioContactDto(
              contactId: _contactId,
              localEnabled: true,
              remoteState: 'enabled',
              state: 'ready',
              changedAtMs: 1,
            ),
            contactName: 'Alice',
          ),
        ),
      ),
    );

    expect(find.byTooltip('Hold to talk'), findsOneWidget);
  });

  testWidgets('Radio enablement lives in the conversation header only', (
    tester,
  ) async {
    final gateway = _RadioGateway(
      const AppSnapshotDto(
        contacts: <ContactDto>[_contact],
        radio: RadioDto(
          contacts: <RadioContactDto>[
            RadioContactDto(
              contactId: _contactId,
              localEnabled: false,
              remoteState: 'enabled',
              state: 'available',
              changedAtMs: 1,
            ),
          ],
        ),
      ),
    );
    addTearDown(gateway.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: ConversationHeader(
            gateway: gateway,
            contact: _contact,
            radio: const RadioContactDto(
              contactId: _contactId,
              localEnabled: false,
              remoteState: 'enabled',
              state: 'available',
              changedAtMs: 1,
            ),
            onConnectionDetails: () {},
          ),
        ),
      ),
    );
    await tester.tap(find.byType(Switch));
    await tester.pump();

    final command = gateway.commands.single;
    expect(command, isA<SetRadioEnabledCommandDto>());
    expect((command as SetRadioEnabledCommandDto).contactIdHex, _contact.id);
    expect(command.enabled, isTrue);

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: ContactDetailsScreen(gateway: gateway, contact: _contact),
      ),
    );
    expect(find.byType(Switch), findsNothing);
  });

  testWidgets(
    'conversation header surface stays readable over scrolled content',
    (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: const Scaffold(
            body: ConversationHeaderSurface(
              child: SizedBox(height: 48, child: Text('Alice')),
            ),
          ),
        ),
      );

      expect(find.byType(BackdropFilter), findsOneWidget);
      final decoration = tester.widget<DecoratedBox>(
        find.descendant(
          of: find.byType(ConversationHeaderSurface),
          matching: find.byType(DecoratedBox),
        ),
      );
      expect(
        (decoration.decoration as BoxDecoration).color?.a,
        greaterThan(0.85),
      );
    },
  );

  testWidgets('desktop conversation header uses one aligned text lane', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: ConversationHeader(
            contact: _contact,
            onConnectionDetails: () {},
          ),
        ),
      ),
    );

    final header = find.byType(ConversationHeader);
    expect(
      find.descendant(of: header, matching: find.byType(Expanded)),
      findsOneWidget,
    );
    expect(
      find.descendant(of: header, matching: find.byType(Spacer)),
      findsNothing,
    );
  });

  testWidgets('conversation header exposes independent RX and TX layers', (
    tester,
  ) async {
    const snapshot = AppSnapshotDto(
      transport: TransportStatusDto(
        tor: TransportIndicatorDto(state: 'ready'),
        relay: TransportIndicatorDto(state: 'healthy'),
        peer: TransportIndicatorDto(state: 'ready'),
      ),
    );
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: ConversationHeader(
            contact: _contact,
            snapshot: snapshot,
            compact: true,
            onConnectionDetails: () {},
          ),
        ),
      ),
    );
    expect(find.byType(RuntimeNetworkStatus), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('communication-status-light')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('peer-status-light')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('relay-status-light')),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('walkie-talkie status bar turns red for TX and RX', (
    tester,
  ) async {
    for (final state in <String>['transmitting', 'receiving']) {
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: Scaffold(
            body: RadioConversationStatus(
              contact: _contact,
              radio: const RadioContactDto(
                contactId: _contactId,
                localEnabled: true,
                remoteState: 'enabled',
                state: 'ready',
                changedAtMs: 1,
              ),
              session: RadioSessionDto(
                contactId: _contactId,
                sessionId: 'radio-session',
                state: state,
                floor: state == 'transmitting' ? 'local' : 'remote',
                burstElapsedMs: 100,
                maxBurstMs: 10000,
              ),
              timeline: const <RadioTimelineEventDto>[],
            ),
          ),
        ),
      );
      final context = tester.element(find.byType(RadioConversationStatus));
      final status = tester.widget<Container>(
        find
            .descendant(
              of: find.byType(RadioConversationStatus),
              matching: find.byType(Container),
            )
            .first,
      );
      expect(status.color, Theme.of(context).colorScheme.errorContainer);
    }
  });

  testWidgets('radio transport failures are visible and provider-neutral', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: const Scaffold(
          body: RadioConversationStatus(
            contact: _contact,
            radio: const RadioContactDto(
              contactId: _contactId,
              localEnabled: true,
              remoteState: 'enabled',
              state: 'reconnecting',
              changedAtMs: 1,
            ),
            session: null,
            timeline: const <RadioTimelineEventDto>[],
            transportFailure: 'connect_timeout',
          ),
        ),
      ),
    );

    expect(find.textContaining('connection timeout'), findsOneWidget);
    expect(find.textContaining('Radio is reconnecting'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('PTT sends begin while held and end on pointer release', (
    tester,
  ) async {
    final gateway = _RadioGateway(const AppSnapshotDto());
    addTearDown(gateway.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: RadioPushToTalk(
            gateway: gateway,
            contact: _contact,
            radio: const RadioContactDto(
              contactId: _contactId,
              localEnabled: true,
              remoteState: 'enabled',
              state: 'ready',
              changedAtMs: 1,
            ),
            session: null,
            requestPermission: () async => true,
          ),
        ),
      ),
    );

    final gesture = await tester.startGesture(
      tester.getCenter(find.byType(RadioPushToTalk)),
    );
    await tester.pump(const Duration(milliseconds: 150));
    expect(
      tester.getSize(find.byKey(const ValueKey<String>('radio-ptt-button'))),
      const Size.square(48),
    );
    expect(gateway.commands, hasLength(1));
    expect(gateway.commands.single, isA<BeginRadioTransmissionCommandDto>());
    final halo = find.byKey(const ValueKey<String>('radio-ptt-halo'));
    expect(halo, findsOneWidget);
    final haloTransform = tester.widget<Transform>(
      find.ancestor(of: halo, matching: find.byType(Transform)).first,
    );
    expect(haloTransform.transform.entry(0, 0), greaterThanOrEqualTo(3.5));

    // Moving past tap slop must not cancel a held transmission. The old tap
    // recognizer released here after losing the gesture arena.
    await gesture.moveBy(const Offset(24, 0));
    await tester.pump(const Duration(milliseconds: 10));
    expect(gateway.commands, hasLength(1));

    await gesture.up();
    await tester.pump(const Duration(milliseconds: 10));
    expect(gateway.commands, hasLength(2));
    expect(gateway.commands.last, isA<EndRadioTransmissionCommandDto>());
  });
}
