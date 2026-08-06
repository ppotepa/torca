import 'package:flutter/foundation.dart';
import '../generated/torca_contract.dart';
import 'engine_gateway.dart';

class MemoryEngineGateway implements EngineGateway {
  final ValueNotifier<AppSnapshotDto> _snapshots = ValueNotifier(const AppSnapshotDto());
  int _sequence = 1;
  @override ValueListenable<AppSnapshotDto> get snapshots => _snapshots;
  String _id() => (_sequence++).toRadixString(16).padLeft(32, '0');

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    final current = _snapshots.value;
    if (command is CreateIdentityCommandDto) {
      if (current.identity != null) return const BridgeResultDto(ok: false, kind: 'error', error: 'identity already exists');
      _snapshots.value = AppSnapshotDto(identity: IdentityDto(displayName: command.displayName), contacts: current.contacts, conversations: current.conversations, messages: current.messages);
      return const BridgeResultDto(ok: true, kind: 'identity_created');
    }
    if (command is StartPairingCommandDto) {
      final contactId = _id(); final conversationId = _id();
      final contact = ContactDto(id: contactId, onionAddress: '${command.code.toLowerCase()}.example.onion', status: 'active');
      final conversation = ConversationDto(id: conversationId, contactId: contactId, status: 'active');
      _snapshots.value = AppSnapshotDto(identity: current.identity, contacts: [...current.contacts, contact], conversations: [...current.conversations, conversation], messages: current.messages);
      return const BridgeResultDto(ok: true, kind: 'pairing_completed');
    }
    if (command is QueueMessageCommandDto) {
      final message = MessageDto(id: command.messageIdHex, conversationId: command.conversationIdHex, body: command.body, direction: 'outbound', status: 'queued');
      _snapshots.value = AppSnapshotDto(identity: current.identity, contacts: current.contacts, conversations: current.conversations, messages: [...current.messages, message]);
      return const BridgeResultDto(ok: true, kind: 'message_queued');
    }
    return const BridgeResultDto(ok: true, kind: 'snapshot');
  }
  @override Future<void> dispose() async { _snapshots.dispose(); }
}
