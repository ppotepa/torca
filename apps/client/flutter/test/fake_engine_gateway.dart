import 'package:flutter/foundation.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';

class FakeEngineGateway implements EngineGateway, PairingUriParser {
  final ValueNotifier<AppSnapshotDto> _snapshots =
      ValueNotifier<AppSnapshotDto>(
        const AppSnapshotDto(torState: 'ready', bootstrapPhase: 'ready'),
      );
  int _sequence = 1;

  @override
  ValueListenable<AppSnapshotDto> get snapshots => _snapshots;

  @override
  Stream<RuntimeEventDto> get events => const Stream<RuntimeEventDto>.empty();

  @override
  Future<void> sendLifecycle(String event) async {}

  @override
  Future<String?> parsePairingUri(String rawUri) async {
    final uri = Uri.tryParse(rawUri);
    if (uri?.scheme != 'torca' ||
        uri?.host != 'pair' ||
        uri?.queryParameters['v'] != '1') {
      return null;
    }
    final code = uri?.queryParameters['code']?.toUpperCase();
    if (code == null || code.length < 6 || code.length > 16) return null;
    if (!RegExp(r'^[A-Z0-9]+$').hasMatch(code)) return null;
    return code;
  }

  @override
  Future<BridgeResultDto> execute(BridgeCommandDto command) async {
    final current = _snapshots.value;
    if (command is UpdateProfileCommandDto) {
      _snapshots.value = _copy(
        current,
        identity: IdentityDto(
          displayName: command.displayName,
          fingerprint: current.identity?.fingerprint,
        ),
      );
      return const BridgeResultDto(ok: true, kind: 'profile_updated');
    }

    if (command is CreatePairingCommandDto ||
        command is JoinPairingCommandDto) {
      final joining = command is JoinPairingCommandDto;
      final id = _newId();
      final code = joining ? command.code : 'TORCA1';
      final pairing = PairingDto(
        id: id,
        code: code,
        role: joining ? 'joiner' : 'creator',
        state: 'open',
        expiresAtMs:
            DateTime.now().millisecondsSinceEpoch +
            const Duration(minutes: 5).inMilliseconds,
        localApproved: false,
        remoteApproved: false,
      );
      _snapshots.value = _copy(
        current,
        pairings: <PairingDto>[...current.pairings, pairing],
      );
      return BridgeResultDto(
        ok: true,
        kind: joining ? 'pairing_joined' : 'pairing_started',
      );
    }

    if (command is ApprovePairingCommandDto) {
      return _updatePairing(
        current,
        command.sessionIdHex,
        (pairing) => PairingDto(
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
      return _terminalPairing(
        current,
        command.sessionIdHex,
        'rejected',
        'pairing_rejected',
      );
    }
    if (command is CancelPairingCommandDto) {
      return _terminalPairing(
        current,
        command.sessionIdHex,
        'cancelled',
        'pairing_cancelled',
      );
    }

    if (command is RenameContactCommandDto) {
      var found = false;
      final name = command.displayName.trim();
      if (name.isEmpty || name.length > 64) {
        return const BridgeResultDto(
          ok: false,
          kind: 'error',
          error: 'invalid contact name',
        );
      }
      final contacts = current.contacts
          .map((contact) {
            if (contact.id != command.contactIdHex) return contact;
            found = true;
            return ContactDto(
              id: contact.id,
              displayName: name,
              onionAddress: contact.onionAddress,
              status: contact.status,
              connectionState: contact.connectionState,
              safetyNumber: contact.safetyNumber,
              peerHealth: contact.peerHealth,
            );
          })
          .toList(growable: false);
      if (!found) {
        return const BridgeResultDto(
          ok: false,
          kind: 'error',
          error: 'contact not found',
        );
      }
      _snapshots.value = _copy(current, contacts: contacts);
      return const BridgeResultDto(ok: true, kind: 'contact_renamed');
    }

    if (command is BlockContactCommandDto ||
        command is UnblockContactCommandDto) {
      final id = command is BlockContactCommandDto
          ? command.contactIdHex
          : (command as UnblockContactCommandDto).contactIdHex;
      final expected = command is BlockContactCommandDto ? 'active' : 'blocked';
      final next = command is BlockContactCommandDto ? 'blocked' : 'active';
      var found = false;
      var valid = false;
      final contacts = current.contacts
          .map((contact) {
            if (contact.id != id) return contact;
            found = true;
            if (contact.status != expected) return contact;
            valid = true;
            final connectionState = next == 'blocked'
                ? 'disconnected'
                : contact.connectionState;
            return ContactDto(
              id: contact.id,
              displayName: contact.displayName,
              onionAddress: contact.onionAddress,
              status: next,
              connectionState: connectionState,
              safetyNumber: contact.safetyNumber,
              peerHealth: next == 'blocked'
                  ? PeerHealthDto(state: connectionState, quality: 'unknown')
                  : contact.peerHealth,
            );
          })
          .toList(growable: false);
      if (!found || !valid) {
        return const BridgeResultDto(
          ok: false,
          kind: 'error',
          error: 'invalid contact transition',
        );
      }
      _snapshots.value = _copy(current, contacts: contacts);
      return BridgeResultDto(
        ok: true,
        kind: next == 'blocked' ? 'contact_blocked' : 'contact_unblocked',
      );
    }

    if (command is RemoveContactCommandDto) {
      final conversations = current.conversations
          .where((item) => item.contactId == command.contactIdHex)
          .map((item) => item.id)
          .toSet();
      if (!current.contacts.any((item) => item.id == command.contactIdHex)) {
        return const BridgeResultDto(
          ok: false,
          kind: 'error',
          error: 'contact not found',
        );
      }
      final messagesToRemove = current.messages
          .where((item) => conversations.contains(item.conversationId))
          .map((item) => item.id)
          .toSet();
      _snapshots.value = _copy(
        current,
        contacts: current.contacts
            .where((item) => item.id != command.contactIdHex)
            .toList(growable: false),
        conversations: current.conversations
            .where((item) => item.contactId != command.contactIdHex)
            .toList(growable: false),
        messages: current.messages
            .where((item) => !conversations.contains(item.conversationId))
            .toList(growable: false),
        attachments: current.attachments
            .where((item) => !messagesToRemove.contains(item.messageId))
            .toList(growable: false),
      );
      return const BridgeResultDto(ok: true, kind: 'contact_removed');
    }

    if (command is ClearConversationHistoryCommandDto) {
      final removed = current.messages
          .where((item) => item.conversationId == command.conversationIdHex)
          .map((item) => item.id)
          .toSet();
      _snapshots.value = _copy(
        current,
        messages: current.messages
            .where((item) => item.conversationId != command.conversationIdHex)
            .toList(growable: false),
        attachments: current.attachments
            .where((item) => !removed.contains(item.messageId))
            .toList(growable: false),
      );
      return const BridgeResultDto(
        ok: true,
        kind: 'conversation_history_cleared',
      );
    }

    if (command is QueueMessageCommandDto) {
      if (command.replyToMessageId != null &&
          !current.messages.any(
            (message) => message.id == command.replyToMessageId,
          )) {
        return const BridgeResultDto(
          ok: false,
          kind: 'error',
          error: 'reply message not found',
        );
      }
      final now = DateTime.now().millisecondsSinceEpoch;
      final message = MessageDto(
        id: _newId(),
        conversationId: command.conversationIdHex,
        body: command.body,
        direction: 'outbound',
        status: 'queued',
        replyToMessageId: command.replyToMessageId,
        createdAtMs: now,
        updatedAtMs: now,
      );
      _snapshots.value = _copy(
        current,
        messages: <MessageDto>[...current.messages, message],
      );
      return const BridgeResultDto(ok: true, kind: 'message_queued');
    }

    if (command is RetryMessageCommandDto) {
      var found = false;
      var invalid = false;
      final now = DateTime.now().millisecondsSinceEpoch;
      final messages = current.messages
          .map((message) {
            if (message.id != command.messageIdHex) return message;
            found = true;
            if (message.direction != 'outbound' || message.status != 'failed') {
              invalid = true;
              return message;
            }
            return MessageDto(
              id: message.id,
              conversationId: message.conversationId,
              body: message.body,
              direction: message.direction,
              status: 'queued',
              replyToMessageId: message.replyToMessageId,
              createdAtMs: message.createdAtMs,
              updatedAtMs: now,
              attemptCount: message.attemptCount,
            );
          })
          .toList(growable: false);
      if (!found || invalid) {
        return const BridgeResultDto(
          ok: false,
          kind: 'error',
          error: 'message is not retryable',
        );
      }
      _snapshots.value = _copy(current, messages: messages);
      return const BridgeResultDto(ok: true, kind: 'message_updated');
    }

    if (command is MarkConversationReadCommandDto) {
      final now = DateTime.now().millisecondsSinceEpoch;
      final messages = current.messages
          .map((message) {
            if (message.conversationId != command.conversationIdHex ||
                message.direction != 'inbound' ||
                message.status != 'delivered') {
              return message;
            }
            return MessageDto(
              id: message.id,
              conversationId: message.conversationId,
              body: message.body,
              direction: message.direction,
              status: 'read',
              replyToMessageId: message.replyToMessageId,
              createdAtMs: message.createdAtMs,
              updatedAtMs: now,
              attemptCount: message.attemptCount,
            );
          })
          .toList(growable: false);
      _snapshots.value = _copy(current, messages: messages);
      return const BridgeResultDto(ok: true, kind: 'conversation_read');
    }

    if (command is QueueAttachmentCommandDto) {
      final messageId = _newId();
      final attachment = AttachmentDto(
        id: _newId(),
        messageId: messageId,
        name: command.name,
        mediaType: command.mediaType,
        size: command.size,
        status: 'queued',
        offset: 0,
      );
      final now = DateTime.now().millisecondsSinceEpoch;
      final message = MessageDto(
        id: messageId,
        conversationId: command.conversationIdHex,
        body: 'Attachment: ${command.name}',
        direction: 'outbound',
        status: 'queued',
        createdAtMs: now,
        updatedAtMs: now,
      );
      _snapshots.value = _copy(
        current,
        messages: <MessageDto>[...current.messages, message],
        attachments: <AttachmentDto>[...current.attachments, attachment],
      );
      return const BridgeResultDto(ok: true, kind: 'attachment_queued');
    }

    return const BridgeResultDto(ok: true, kind: 'snapshot');
  }

  @override
  Future<String> diagnosticsJson() async =>
      '{"events":[{"component":"Engine","state":"Ready","code":"MEMORY_PREVIEW"}]}';

  BridgeResultDto _terminalPairing(
    AppSnapshotDto current,
    String id,
    String state,
    String kind,
  ) => _updatePairing(
    current,
    id,
    (pairing) => PairingDto(
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

  BridgeResultDto _updatePairing(
    AppSnapshotDto current,
    String id,
    PairingDto Function(PairingDto) update,
    String kind,
  ) {
    var found = false;
    final pairings = current.pairings
        .map((pairing) {
          if (pairing.id != id) return pairing;
          found = true;
          return update(pairing);
        })
        .toList(growable: false);
    if (!found) {
      return const BridgeResultDto(
        ok: false,
        kind: 'error',
        error: 'pairing session not found',
      );
    }
    _snapshots.value = _copy(current, pairings: pairings);
    return BridgeResultDto(ok: true, kind: kind);
  }

  AppSnapshotDto _copy(
    AppSnapshotDto current, {
    IdentityDto? identity,
    List<PairingDto>? pairings,
    List<ContactDto>? contacts,
    List<ConversationDto>? conversations,
    List<MessageDto>? messages,
    List<AttachmentDto>? attachments,
  }) => AppSnapshotDto(
    identity: identity ?? current.identity,
    torState: current.torState,
    onionAddress: current.onionAddress,
    pairings: pairings ?? current.pairings,
    contacts: contacts ?? current.contacts,
    conversations: conversations ?? current.conversations,
    messages: messages ?? current.messages,
    attachments: attachments ?? current.attachments,
    bootstrapPhase: current.bootstrapPhase,
    bootstrapSteps: current.bootstrapSteps,
  );

  String _newId() => (_sequence++).toRadixString(16).padLeft(32, '0');

  @override
  Future<void> dispose() async {
    _snapshots.dispose();
  }
}
