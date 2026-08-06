import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/method_channel_engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const MethodChannel channel = MethodChannel(
    MethodChannelEngineGateway.channelName,
  );

  tearDown(() async {
    await TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test('initial snapshot and command use the versioned native channel', () async {
    final List<MethodCall> calls = <MethodCall>[];
    Map<String, Object?> snapshot = <String, Object?>{
      'contractVersion': torcaContractVersion,
      'identity': <String, Object?>{'displayName': 'Native Orca'},
      'contacts': <Object?>[],
      'conversations': <Object?>[],
      'messages': <Object?>[],
    };

    await TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (MethodCall call) async {
      calls.add(call);
      if (call.method == 'snapshot') {
        return snapshot;
      }
      if (call.method == 'execute') {
        final Map<Object?, Object?> envelope =
            call.arguments! as Map<Object?, Object?>;
        final Map<Object?, Object?> command =
            envelope['command']! as Map<Object?, Object?>;
        expect(envelope['contractVersion'], torcaContractVersion);
        expect(command['type'], 'queueMessage');
        snapshot = <String, Object?>{
          ...snapshot,
          'messages': <Object?>[
            <String, Object?>{
              'id': command['messageIdHex']!,
              'conversationId': command['conversationIdHex']!,
              'body': command['body']!,
              'direction': 'outbound',
              'status': 'queued',
            },
          ],
        };
        return <String, Object?>{
          'ok': true,
          'kind': 'message_queued',
          'error': null,
        };
      }
      throw PlatformException(code: 'unsupported_method');
    });

    final MethodChannelEngineGateway gateway =
        MethodChannelEngineGateway(channel: channel);
    final BridgeResultDto initialized = await gateway.initialize();

    expect(initialized.ok, isTrue);
    expect(gateway.snapshots.value.identity?.displayName, 'Native Orca');

    final BridgeResultDto result = await gateway.execute(
      const QueueMessageCommandDto(
        messageIdHex: '00000000000000000000000000000001',
        conversationIdHex: '00000000000000000000000000000002',
        body: 'hello',
        atMs: 0,
      ),
    );

    expect(result.kind, 'message_queued');
    expect(gateway.snapshots.value.messages.single.body, 'hello');
    expect(calls.map((MethodCall call) => call.method), <String>[
      'snapshot',
      'execute',
      'snapshot',
    ]);

    await gateway.dispose();
  });

  test('missing native host returns a typed error', () async {
    final MethodChannelEngineGateway gateway =
        MethodChannelEngineGateway(channel: channel);

    final BridgeResultDto result = await gateway.initialize();

    expect(result.ok, isFalse);
    expect(result.error, contains('native Torca engine host is unavailable'));
    await gateway.dispose();
  });
}
