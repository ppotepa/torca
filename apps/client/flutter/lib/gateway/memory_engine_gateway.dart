import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';
import 'engine_gateway.dart';

class MemoryEngineGateway implements EngineGateway {
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto(torState: 'ready'));

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    final current = _snapshots.value;
    if (command is CreateIdentityCommandDto) {
      if (current.identity != null) {
        return const BridgeResultDto(ok: false, kind: 'error', error: 'identity already exists');
      }
      _snapshots.value = _copy(current, identity: IdentityDto(displayName: command.displayName));
      return const BridgeResultDto(ok: true, kind: 'identity_created');
    }
    if (command is CreatePairingCommandDto || command is JoinPairingCommandDto) {
      final joining = command is JoinPairingCommandDto;
      final id = joining ? command.sessionIdHex : (command as CreatePairingCommandDto).sessionIdHex;
      final code = joining ? command.code : 'TORCA1';
      if (current.pairings.any((item) => item.id == id)) {
        return const BridgeResultDto(ok: false, kind: 'error', error: 'pairing already exists');
      }
      final pairing = PairingDto(
        id: id,
        code: code,
        role: joining ? 'joiner' : 'creator',
        state: 'open',
        expiresAtMs: DateTime.now().millisecondsSinceEpoch + const Duration(minutes: 5).inMilliseconds,
        localApproved: false,
        remoteApproved: false,
      );
      _snapshots.value = _copy(current, pairings: <PairingDto>[...current.pairings, pairing]);
      return BridgeResultDto(ok: true, kind: joining ? 'pairing_joined' : 'pairing_started');
    }
    if (command is ApprovePairingCommandDto) {
      return _updatePairing(current, command.sessionIdHex, (pairing) => PairingDto(
        id: pairing.id,
        code: pairing.code,
        role: pairing.role,
        state: 'approved',
        expiresAtMs: pairing.expiresAtMs,
        localApproved: true,
        remoteApproved: pairing.remoteApproved,
      ), 'pairing_updated');
    }
    if (command is RejectPairingCommandDto) {
      return _terminalPairing(current, command.sessionIdHex, 'rejected', 'pairing_rejected');
    }
    if (command is CancelPairingCommandDto) {
      return _terminalPairing(current, command.sessionIdHex, 'cancelled', 'pairing_cancelled');
    }
    if (command is QueueMessageCommandDto) {
      final message = MessageDto(
        id: command.messageIdHex,
        conversationId: command.conversationIdHex,
        body: command.body,
        direction: 'outbound',
        status: 'queued',
      );
      _snapshots.value = _copy(current, messages: <MessageDto>[...current.messages, message]);
      return const BridgeResultDto(ok: true, kind: 'message_queued');
    }
    if (command is MarkConversationReadCommandDto) {
      final messages = current.messages.map((message) {
        if (message.conversationId != command.conversationIdHex ||
            message.direction != 'inbound' || message.status != 'delivered') {
          return message;
        }
        return MessageDto(
          id: message.id,
          conversationId: message.conversationId,
          body: message.body,
          direction: message.direction,
          status: 'read',
        );
      }).toList(growable: false);
      _snapshots.value = _copy(current, messages: messages);
      return const BridgeResultDto(ok: true, kind: 'conversation_read');
    }
    return const BridgeResultDto(ok: true, kind: 'snapshot');
  }

  @override
  Future<String> diagnosticsJson() async =>
      '{"events":[{"component":"Engine","state":"Ready","code":"MEMORY_PREVIEW"}]}';

  BridgeResultDto _terminalPairing(AppSnapshotDto current, String id, String state, String kind) {
    return _updatePairing(current, id, (pairing) => PairingDto(
      id: pairing.id,
      code: pairing.code,
      role: pairing.role,
      state: state,
      expiresAtMs: pairing.expiresAtMs,
      localApproved: pairing.localApproved,
      remoteApproved: pairing.remoteApproved,
    ), kind);
  }

  BridgeResultDto _updatePairing(
    AppSnapshotDto current,
    String id,
    PairingDto Function(PairingDto) update,
    String kind,
  ) {
    var found = false;
    final pairings = current.pairings.map((pairing) {
      if (pairing.id != id) return pairing;
      found = true;
      return update(pairing);
    }).toList(growable: false);
    if (!found) {
      return const BridgeResultDto(ok: false, kind: 'error', error: 'pairing session not found');
    }
    _snapshots.value = _copy(current, pairings: pairings);
    return BridgeResultDto(ok: true, kind: kind);
  }

  AppSnapshotDto _copy(
    AppSnapshotDto current, {
    IdentityDto? identity,
    List<PairingDto>? pairings,
    List<MessageDto>? messages,
  }) => AppSnapshotDto(
    identity: identity ?? current.identity,
    torState: current.torState,
    onionAddress: current.onionAddress,
    pairings: pairings ?? current.pairings,
    contacts: current.contacts,
    conversations: current.conversations,
    messages: messages ?? current.messages,
  );

  @override
  Future<void> dispose() async { _snapshots.dispose(); }
}
