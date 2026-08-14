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
    return const BridgeResultDto(ok: true, kind: 'accepted');
  }

  @override
  Future<void> sendLifecycle(String event) async {}

  @override
  Future<String> diagnosticsJson() async => '{"events":[]}';

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
    expect(gateway.commands, hasLength(1));
    expect(gateway.commands.single, isA<BeginRadioTransmissionCommandDto>());

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
