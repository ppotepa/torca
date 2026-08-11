import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/controllers/pairing_action_controller.dart';
import 'package:torca_app/generated/torca_contract.dart';

import 'fake_engine_gateway.dart';

void main() {
  test('pairing action maps to one typed bridge command', () async {
    final gateway = FakeEngineGateway();
    final controller = PairingActionController(gateway);

    expect(await controller.run(PairingAction.approve, 'a1'), isTrue);
    expect(gateway.commands, hasLength(1));
    expect(gateway.commands.single, isA<ApprovePairingCommandDto>());
    expect(
      (gateway.commands.single as ApprovePairingCommandDto).sessionIdHex,
      'a1',
    );

    controller.dispose();
  });

  test('pairing action exposes a presentation-safe failure', () async {
    final gateway = FakeEngineGateway(
      responses: const <FakeGatewayResponse>[
        FakeGatewayResponse(
          result: BridgeResultDto(
            ok: false,
            kind: 'error',
            errorCode: 'relay_unavailable',
            messageKey: 'error.relay.unavailable',
          ),
        ),
      ],
    );
    final controller = PairingActionController(gateway);

    expect(await controller.run(PairingAction.reject, 'b2'), isFalse);
    expect(controller.busy, isFalse);
    expect(controller.error, isNot(contains('error.relay.unavailable')));

    controller.dispose();
  });
}
