import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  test('bridge contract v10 carries nested peer health', () {
    expect(torcaContractVersion, 10);

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

  test('peer health defaults are safe before a sample exists', () {
    const health = PeerHealthDto();
    expect(health.state, 'disconnected');
    expect(health.quality, 'unknown');
    expect(health.rttMs, isNull);
    expect(health.consecutiveFailures, 0);
  });
}
