import 'package:flutter/foundation.dart';

import '../generated/torca_contract.dart';
import 'engine_gateway.dart';

class MemoryEngineGateway implements EngineGateway {
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(const AppSnapshotDto());

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    final AppSnapshotDto current = _snapshots.value;
    if (command is CreateIdentityCommandDto) {
      if (current.identity != null) {
        return const BridgeResultDto(ok: false, kind: 'error', error: 'identity already exists');
      }
      _snapshots.value = AppSnapshotDto(
        identity: IdentityDto(displayName: command.displayName),
        pairings: current.pairings,
        contacts: current.contacts,
        conversations: current.conversations,
        messages: current.messages,
      );
      return const BridgeResultDto(ok: true, kind: 'identity_created');
    }

    if (command is StartPairingCommandDto || command is JoinPairingCommandDto) {
      final bool joining = command is JoinPairingCommandDto;
      final String sessionId = joining
          ? (command as JoinPairingCommandDto).sessionIdHex
          : (command as StartPairingCommandDto).sessionIdHex;
      final String code = joining
          ? (command as JoinPairingCommandDto).code
          : (command as StartPairingCommandDto).code;
      final int expiresAtMs = joining
          ? (command as JoinPairingCommandDto).expiresAtMs
          : (command as StartPairingCommandDto).expiresAtMs;
      if (current.pairings.any((PairingDto item) => item.id == sessionId)) {
        return const BridgeResultDto(ok: false, kind: 'error', error: 'pairing already exists');
      }
      final PairingDto pairing = PairingDto(
        id: sessionId,
        code: code,
        role: joining ? 'joiner' : 'creator',
        state: 'open',
        expiresAtMs: expiresAtMs,
        localApproved: false,
        remoteApproved: false,
      );
      _snapshots.value = _copy(current, pairings: <PairingDto>[...current.pairings, pairing]);
      return BridgeResultDto(ok: true, kind: joining ? 'pairing_joined' : 'pairing_started');
    }

    if (command is ApprovePairingCommandDto) {
      return _updatePairing(
        current,
        command.sessionIdHex,
        (PairingDto pairing) => PairingDto(
          id: pairing.id,
          code: pairing.code,
          role: pairing.role,
          state: 'approved',
          expiresAtMs: pairing.expiresAtMs,
          localApproved: true,
          remoteApproved: pairing.remoteApproved,
        ),
        'pairing_updated',
      );
    }

    if (command is RejectPairingCommandDto) {
      return _terminalPairing(current, command.sessionIdHex, 'rejected', 'pairing_rejected');
    }
    if (command is CancelPairingCommandDto) {
      return _terminalPairing(current, command.sessionIdHex, 'cancelled', 'pairing_cancelled');
    }

    if (command is QueueMessageCommandDto) {
      final MessageDto message = MessageDto(
        id: command.messageIdHex,
        conversationId: command.conversationIdHex,
        body: command.body,
        direction: 'outbound',
        status: 'queued',
      );
      _snapshots.value = _copy(
        current,
        messages: <MessageDto>[...current.messages, message],
      );
      return const BridgeResultDto(ok: true, kind: 'message_queued');
    }

    return const BridgeResultDto(ok: true, kind: 'snapshot');
  }

  BridgeResultDto _terminalPairing(
    AppSnapshotDto current,
    String id,
    String state,
    String kind,
  ) {
    return _updatePairing(
      current,
      id,
      (PairingDto pairing) => PairingDto(
        id: pairing.id,
        code: pairing.code,
        role: pairing.role,
        state: state,
        expiresAtMs: pairing.expiresAtMs,
        localApproved: pairing.localApproved,
        remoteApproved: pairing.remoteApproved,
      ),
      kind,
    );
  }

  BridgeResultDto _updatePairing(
    AppSnapshotDto current,
    String id,
    PairingDto Function(PairingDto) update,
    String kind,
  ) {
    bool found = false;
    final List<PairingDto> pairings = current.pairings.map((PairingDto pairing) {
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
    List<PairingDto>? pairings,
    List<MessageDto>? messages,
  }) {
    return AppSnapshotDto(
      identity: current.identity,
      pairings: pairings ?? current.pairings,
      contacts: current.contacts,
      conversations: current.conversations,
      messages: messages ?? current.messages,
    );
  }

  @override
  Future<void> dispose() async {
    _snapshots.dispose();
  }
}
