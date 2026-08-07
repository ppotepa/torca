import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  test('bridge contract v11 carries nested peer health', () {
    expect(torcaContractVersion, 11);

    const health = PeerHealthDto(
      state: 'ready',
      quality: 'good',
      rttMs: 721,
      lastSuccessAtMs: 1234,
      consecutiveFailures: 0,
      reconnectAttempt: 2,
    );
    const contact = ContactDto(
      id: '00000000000000000000000000000001',
      onionAddress: 'example.onion',
      status: 'active',
      connectionState: 'ready',
      peerHealth: health,
    );

    expect(contact.peerHealth.quality, 'good');
    expect(contact.peerHealth.rttMs, 721);
    expect(contact.peerHealth.reconnectAttempt, 2);
  });

  test('v11 creation commands carry user intent only', () {
    const identity = CreateIdentityCommandDto(displayName: 'Alice');
    const pairing = CreatePairingCommandDto();
    const message = QueueMessageCommandDto(
      conversationIdHex: '00000000000000000000000000000001',
      body: 'hello',
    );
    const attachment = QueueAttachmentCommandDto(
      conversationIdHex: '00000000000000000000000000000001',
      sourcePath: '/tmp/file',
      name: 'file',
      mediaType: 'application/octet-stream',
      size: 1,
    );

    expect(identity.displayName, 'Alice');
    expect(pairing, isA<CreatePairingCommandDto>());
    expect(message.body, 'hello');
    expect(attachment.size, 1);
  });

  test('peer health defaults are safe before a sample exists', () {
    const health = PeerHealthDto();
    expect(health.state, 'disconnected');
    expect(health.quality, 'unknown');
    expect(health.rttMs, isNull);
    expect(health.consecutiveFailures, 0);
  });
}
