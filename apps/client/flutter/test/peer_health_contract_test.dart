import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  test(
    'current bridge contract carries bootstrap progress and navigation badges',
    () {
      expect(torcaContractVersion, greaterThan(0));

      const badges = NavigationBadgesDto(
        unreadMessages: 3,
        newContacts: 1,
        pairingAttention: 2,
      );
      expect(badges.unreadMessages, 3);
      expect(badges.newContacts, 1);

      const bootstrap = BootstrapStepDto(
        id: 'communication_runtime',
        state: 'running',
        code: 'TOR_DIRECTORY_CONSENSUS',
        progress: 15,
        attempt: 1,
        startedAtMs: 1000,
        lastProgressAtMs: 2000,
      );
      expect(bootstrap.progress, 15);
      expect(bootstrap.attempt, 1);

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
        displayName: 'Peer',
        onionAddress: 'example.onion',
        status: 'active',
        connectionState: 'ready',
        peerHealth: health,
      );

      expect(contact.peerHealth.quality, 'good');
      expect(contact.peerHealth.rttMs, 721);
      expect(contact.peerHealth.reconnectAttempt, 2);
    },
  );

  test('runtime commands carry user intent only', () {
    const pairing = CreatePairingCommandDto();
    const profile = UpdateProfileCommandDto(displayName: 'Alice');
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

    expect(profile.displayName, 'Alice');
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

  test(
    'provider-neutral readiness and contact routes decode without onion data',
    () {
      final snapshot = AppSnapshotDto.fromJson(<String, dynamic>{
        'communicationProvider': 'webrtc',
        'communicationState': 'ready',
        'endpointSummary': 'signalling ready',
        'transport': <String, dynamic>{
          'communication': <String, dynamic>{'state': 'ready'},
        },
        'contacts': <Map<String, dynamic>>[
          <String, dynamic>{
            'id': '00000000000000000000000000000001',
            'remoteIdentityId': '00000000000000000000000000000002',
            'displayName': 'Peer',
            'onionAddress': '',
            'transportProvider': 'webrtc',
            'endpointAvailable': true,
            'status': 'active',
            'connectionState': 'ready',
          },
        ],
      });

      expect(snapshot.communicationProvider, 'webrtc');
      expect(snapshot.transport.communication.state, 'ready');
      expect(snapshot.contacts.single.transportProvider, 'webrtc');
      expect(snapshot.contacts.single.endpointAvailable, isTrue);
    },
  );
}
