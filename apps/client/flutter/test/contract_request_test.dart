import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  test('generated contract encodes every supported command', () {
    final commands = <BridgeCommandDto>[
      const UpdateProfileCommandDto(displayName: 'Alice'),
      const CreatePairingCommandDto(),
      const JoinPairingCommandDto(code: 'ABC123'),
      const ApprovePairingCommandDto(sessionIdHex: '01'),
      const RejectPairingCommandDto(sessionIdHex: '01'),
      const CancelPairingCommandDto(sessionIdHex: '01'),
      const RenameContactCommandDto(contactIdHex: '02', displayName: 'Bob'),
      const VerifyContactCommandDto(contactIdHex: '02'),
      const ResetContactVerificationCommandDto(contactIdHex: '02'),
      const BlockContactCommandDto(contactIdHex: '02'),
      const UnblockContactCommandDto(contactIdHex: '02'),
      const RemoveContactCommandDto(contactIdHex: '02'),
      const ClearConversationHistoryCommandDto(conversationIdHex: '03'),
      const QueueMessageCommandDto(conversationIdHex: '03', body: 'hello'),
      const RetryMessageCommandDto(messageIdHex: '04'),
      const MarkConversationReadCommandDto(conversationIdHex: '03'),
      const QueueAttachmentCommandDto(
        conversationIdHex: '03',
        sourcePath: '/tmp/file',
        name: 'file',
        mediaType: 'text/plain',
        size: 1,
      ),
      const RetryAttachmentCommandDto(attachmentIdHex: '05'),
      const CancelAttachmentCommandDto(attachmentIdHex: '05'),
      const ExportAttachmentCommandDto(
        attachmentIdHex: '05',
        destinationPath: '/tmp/out',
      ),
      const SetNotificationsCommandDto(enabled: true),
      const AcknowledgeNewContactsCommandDto(),
      const RefreshSnapshotCommandDto(),
    ];

    for (final command in commands) {
      final request = RuntimeRequestDto.command(command);
      expect(request, isNotNull);
      final wire =
          jsonDecode(request!.encode('request-1')) as Map<String, dynamic>;
      expect(wire['schema'], 1);
      expect(wire['requestId'], 'request-1');
      expect(wire['name'], isNotEmpty);
    }
  });

  test('generated contract encodes runtime queries and lifecycle', () {
    final requests = <RuntimeRequestDto>[
      RuntimeRequestDto.snapshot,
      RuntimeRequestDto.lifecycle('foregrounded'),
      RuntimeRequestDto.pairingParse('torca://pair?v=1&code=ABC123'),
      RuntimeRequestDto.conversationPage('03', limit: 100),
      RuntimeRequestDto.conversationSearch('03', query: 'hello', limit: 100),
      RuntimeRequestDto.notificationEvents(7),
    ];

    for (final request in requests) {
      final wire =
          jsonDecode(request.encode('request-2')) as Map<String, dynamic>;
      expect(wire['kind'], isNotEmpty);
      expect(wire['name'], isNotEmpty);
      expect(wire['payload'], isA<Map<String, dynamic>>());
    }
  });

  test('notification event decodes the millisecond timestamp field', () {
    final event = RuntimeEventDto.fromJson(<String, dynamic>{
      'cursor': 11,
      'eventId': 'event-11',
      'kind': 'message_received',
      'conversationId': 'conversation-1',
      'contactDisplayName': 'Alice',
      'createdAtMs': 1700000000123,
      'createdAt': 1,
    });
    expect(event.createdAtMs, 1700000000123);
  });
}
