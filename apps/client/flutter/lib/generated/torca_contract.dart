// GENERATED FILE. DO NOT EDIT.
// Generated from: crates/platform/torca-contract/schema/torca_contract.json

import 'dart:convert';

const int torcaContractVersion = 17;
const int torcaNativeAbiVersion = 1;

class BridgeResultDto {
  const BridgeResultDto({
    required this.ok,
    required this.kind,
    this.error,
    String? errorCode,
    this.resourceId,
    this.inviteUri,
  }) : _wireErrorCode = errorCode;
  final bool ok;
  final String kind;
  final String? error;
  final String? resourceId;
  final String? inviteUri;
  final String? _wireErrorCode;
  String? get errorCode =>
      _wireErrorCode ?? (kind.startsWith('error:') ? kind.substring(6) : null);
}

class IdentityDto {
  const IdentityDto({this.displayName, this.fingerprint});
  final String? displayName;
  final String? fingerprint;
}

class BootstrapStepDto {
  const BootstrapStepDto({
    required this.id,
    required this.state,
    this.code,
    this.progress = 0,
    this.attempt = 0,
    this.startedAtMs,
    this.lastProgressAtMs,
    this.retryAtMs,
  });
  final String id, state;
  final String? code;
  final int progress, attempt;
  final int? startedAtMs, lastProgressAtMs, retryAtMs;
}

enum PairingRole { creator, joiner, unknown }

enum PairingState {
  open,
  peerJoined,
  awaitingApproval,
  approved,
  completed,
  rejected,
  cancelled,
  expired,
  unknown,
}

class PairingDto {
  const PairingDto({
    required this.id,
    required this.code,
    required this.inviteUri,
    required this.role,
    required this.state,
    required this.expiresAtMs,
    required this.localApproved,
    required this.remoteApproved,
    this.remoteIdentityId,
    this.remoteDisplayName,
    this.remoteFingerprint,
  });
  final String id, code, inviteUri, role, state;
  final int expiresAtMs;
  final bool localApproved, remoteApproved;
  final String? remoteIdentityId, remoteDisplayName, remoteFingerprint;

  PairingRole get typedRole => switch (role) {
    'creator' => PairingRole.creator,
    'joiner' => PairingRole.joiner,
    _ => PairingRole.unknown,
  };

  PairingState get typedState => switch (state) {
    'open' => PairingState.open,
    'peerjoined' => PairingState.peerJoined,
    'awaitingapproval' => PairingState.awaitingApproval,
    'approved' => PairingState.approved,
    'completed' => PairingState.completed,
    'rejected' => PairingState.rejected,
    'cancelled' => PairingState.cancelled,
    'expired' => PairingState.expired,
    _ => PairingState.unknown,
  };
}

class PeerHealthDto {
  const PeerHealthDto({
    this.state = 'disconnected',
    this.quality = 'unknown',
    this.rttMs,
    this.lastSuccessAtMs,
    this.consecutiveFailures = 0,
    this.reconnectAttempt = 0,
    this.lastActivityAtMs,
    this.activitySequence = 0,
  });
  final String state, quality;
  final int? rttMs, lastSuccessAtMs, lastActivityAtMs;
  final int consecutiveFailures, reconnectAttempt, activitySequence;
}

class TransportIndicatorDto {
  const TransportIndicatorDto({
    this.state = 'unknown',
    this.code = 'UNAVAILABLE',
    this.latencyMs,
    this.lastActivityAtMs,
    this.activitySequence = 0,
    this.txSequence = 0,
    this.rxSequence = 0,
    this.inFlight = 0,
    this.queued = 0,
  });
  final String state, code;
  final int? latencyMs, lastActivityAtMs;
  final int activitySequence, txSequence, rxSequence, inFlight, queued;

  bool get isUsable => state == 'healthy' || state == 'ready';
}

class TransportStatusDto {
  const TransportStatusDto({
    this.tor = const TransportIndicatorDto(state: 'stopped'),
    this.relay = const TransportIndicatorDto(),
    this.peer = const TransportIndicatorDto(state: 'disconnected'),
    this.peersReady = 0,
    this.peersTotal = 0,
    this.relayInfo,
  });
  final TransportIndicatorDto tor, relay, peer;
  final int peersReady, peersTotal;
  final RelayInfoDto? relayInfo;
}

class RelayInfoDto {
  const RelayInfoDto({
    required this.productVersion,
    required this.buildId,
    required this.sourceCommit,
    required this.protocolVersion,
  });
  final String productVersion, buildId, sourceCommit;
  final int protocolVersion;
}

class NavigationBadgesDto {
  const NavigationBadgesDto({
    this.unreadMessages = 0,
    this.newContacts = 0,
    this.pairingAttention = 0,
  });
  final int unreadMessages, newContacts, pairingAttention;
}

class ContactDto {
  const ContactDto({
    required this.id,
    this.displayName = 'Contact',
    required this.onionAddress,
    required this.status,
    required this.connectionState,
    this.presenceState = 'unknown',
    this.lastSeenAtMs,
    this.safetyNumber,
    this.peerHealth = const PeerHealthDto(),
    this.verificationStatus = 'unverified',
    this.verifiedAtMs,
  });
  final String id,
      displayName,
      onionAddress,
      status,
      connectionState,
      presenceState,
      verificationStatus;
  final String? safetyNumber;
  final PeerHealthDto peerHealth;
  final int? verifiedAtMs;
  final int? lastSeenAtMs;
}

class ConversationDto {
  const ConversationDto({
    required this.id,
    required this.contactId,
    required this.status,
    this.unreadCount = 0,
    this.lastActivityAtMs = 0,
    this.lastMessageBody,
    this.lastMessageDirection,
    this.lastMessageStatus,
  });
  final String id, contactId, status;
  final int unreadCount, lastActivityAtMs;
  final String? lastMessageBody, lastMessageDirection, lastMessageStatus;
}

class MessageDto {
  const MessageDto({
    required this.id,
    required this.conversationId,
    required this.body,
    required this.direction,
    required this.status,
    this.replyToMessageId,
    this.createdAtMs = 0,
    this.updatedAtMs = 0,
    this.sentAtMs,
    this.deliveredAtMs,
    this.readAtMs,
    this.attemptCount = 0,
  });
  final String id, conversationId, body, direction, status;
  final String? replyToMessageId;
  final int createdAtMs, updatedAtMs, attemptCount;
  final int? sentAtMs, deliveredAtMs, readAtMs;
}

class AttachmentDto {
  const AttachmentDto({
    required this.id,
    required this.messageId,
    required this.name,
    required this.mediaType,
    required this.size,
    required this.status,
    required this.offset,
    this.attemptCount = 0,
    this.updatedAtMs = 0,
    this.direction = 'outbound',
  });
  final String id, messageId, name, mediaType, status;
  final int size, offset, attemptCount, updatedAtMs;
  final String direction;
}

class PendingOperationDto {
  const PendingOperationDto({
    required this.id,
    required this.resourceId,
    required this.kind,
    required this.state,
    required this.dependency,
    required this.attempts,
    required this.nextAttemptAtMs,
    required this.createdAtMs,
    this.lastError,
  });
  final String id, resourceId, kind, state, dependency;
  final int attempts, nextAttemptAtMs, createdAtMs;
  final String? lastError;
}

class AppSnapshotDto {
  const AppSnapshotDto({
    this.runtimeId = '',
    this.revision = 0,
    this.notificationCursor = 0,
    this.notificationsEnabled = true,
    this.identity,
    this.torState = 'stopped',
    this.transport = const TransportStatusDto(),
    this.navigationBadges = const NavigationBadgesDto(),
    this.onionAddress,
    this.pairings = const [],
    this.contacts = const [],
    this.conversations = const [],
    this.messages = const [],
    this.attachments = const [],
    this.pendingOperations = const [],
    this.bootstrapPhase = 'failed',
    this.bootstrapSteps = const [],
  });
  final String runtimeId;
  final int revision, notificationCursor;
  final bool notificationsEnabled;
  final IdentityDto? identity;
  final String torState;
  final TransportStatusDto transport;
  final NavigationBadgesDto navigationBadges;
  final String? onionAddress;
  final List<PairingDto> pairings;
  final List<ContactDto> contacts;
  final List<ConversationDto> conversations;
  final List<MessageDto> messages;
  final List<AttachmentDto> attachments;
  final List<PendingOperationDto> pendingOperations;
  final String bootstrapPhase;
  final List<BootstrapStepDto> bootstrapSteps;
}

sealed class BridgeCommandDto {
  const BridgeCommandDto();
}

class UpdateProfileCommandDto extends BridgeCommandDto {
  const UpdateProfileCommandDto({required this.displayName});
  final String displayName;
}

class CreatePairingCommandDto extends BridgeCommandDto {
  const CreatePairingCommandDto();
}

class JoinPairingCommandDto extends BridgeCommandDto {
  const JoinPairingCommandDto({required this.code, this.ticket});
  final String code;
  final String? ticket;
}

class ApprovePairingCommandDto extends BridgeCommandDto {
  const ApprovePairingCommandDto({required this.sessionIdHex});
  final String sessionIdHex;
}

class RejectPairingCommandDto extends BridgeCommandDto {
  const RejectPairingCommandDto({required this.sessionIdHex});
  final String sessionIdHex;
}

class CancelPairingCommandDto extends BridgeCommandDto {
  const CancelPairingCommandDto({required this.sessionIdHex});
  final String sessionIdHex;
}

class RenameContactCommandDto extends BridgeCommandDto {
  const RenameContactCommandDto({
    required this.contactIdHex,
    required this.displayName,
  });
  final String contactIdHex, displayName;
}

class VerifyContactCommandDto extends BridgeCommandDto {
  const VerifyContactCommandDto({required this.contactIdHex});
  final String contactIdHex;
}

class ResetContactVerificationCommandDto extends BridgeCommandDto {
  const ResetContactVerificationCommandDto({required this.contactIdHex});
  final String contactIdHex;
}

class BlockContactCommandDto extends BridgeCommandDto {
  const BlockContactCommandDto({required this.contactIdHex});
  final String contactIdHex;
}

class UnblockContactCommandDto extends BridgeCommandDto {
  const UnblockContactCommandDto({required this.contactIdHex});
  final String contactIdHex;
}

class RemoveContactCommandDto extends BridgeCommandDto {
  const RemoveContactCommandDto({required this.contactIdHex});
  final String contactIdHex;
}

class StartConversationCommandDto extends BridgeCommandDto {
  const StartConversationCommandDto({required this.contactIdHex});
  final String contactIdHex;
}

class ClearConversationHistoryCommandDto extends BridgeCommandDto {
  const ClearConversationHistoryCommandDto({required this.conversationIdHex});
  final String conversationIdHex;
}

class QueueMessageCommandDto extends BridgeCommandDto {
  const QueueMessageCommandDto({
    required this.conversationIdHex,
    required this.body,
    this.replyToMessageId,
  });
  final String conversationIdHex, body;
  final String? replyToMessageId;
}

class RetryMessageCommandDto extends BridgeCommandDto {
  const RetryMessageCommandDto({required this.messageIdHex});
  final String messageIdHex;
}

class MarkConversationReadCommandDto extends BridgeCommandDto {
  const MarkConversationReadCommandDto({required this.conversationIdHex});
  final String conversationIdHex;
}

class QueueAttachmentCommandDto extends BridgeCommandDto {
  const QueueAttachmentCommandDto({
    required this.conversationIdHex,
    required this.sourcePath,
    required this.name,
    required this.mediaType,
    required this.size,
  });
  final String conversationIdHex, sourcePath, name, mediaType;
  final int size;
}

class RetryAttachmentCommandDto extends BridgeCommandDto {
  const RetryAttachmentCommandDto({required this.attachmentIdHex});
  final String attachmentIdHex;
}

class CancelAttachmentCommandDto extends BridgeCommandDto {
  const CancelAttachmentCommandDto({required this.attachmentIdHex});
  final String attachmentIdHex;
}

class ExportAttachmentCommandDto extends BridgeCommandDto {
  const ExportAttachmentCommandDto({
    required this.attachmentIdHex,
    required this.destinationPath,
  });
  final String attachmentIdHex, destinationPath;
}

class SetNotificationsCommandDto extends BridgeCommandDto {
  const SetNotificationsCommandDto({required this.enabled});
  final bool enabled;
}

class AcknowledgeNewContactsCommandDto extends BridgeCommandDto {
  const AcknowledgeNewContactsCommandDto();
}

class RefreshSnapshotCommandDto extends BridgeCommandDto {
  const RefreshSnapshotCommandDto();
}

/// Generated wire encoder for the only supported native ABI operation.
///
/// Keeping the command-to-wire mapping here makes the canonical contract the
/// sole owner of operation names and payload keys. Presentation code supplies
/// typed DTOs only; it does not hand-assemble native JSON.
class RuntimeRequestDto {
  const RuntimeRequestDto._({
    required this.kind,
    required this.name,
    required this.payload,
  });

  factory RuntimeRequestDto.lifecycle(String event) => RuntimeRequestDto._(
    kind: 'lifecycle',
    name: event,
    payload: const <String, Object?>{},
  );

  factory RuntimeRequestDto.pairingParse(String rawUri) => RuntimeRequestDto._(
    kind: 'query',
    name: 'pairing.parse',
    payload: <String, Object?>{'uri': rawUri},
  );

  factory RuntimeRequestDto.pairingEncode(String code) => RuntimeRequestDto._(
    kind: 'query',
    name: 'pairing.encode',
    payload: <String, Object?>{'code': code},
  );

  factory RuntimeRequestDto.conversationPage(
    String conversationId, {
    String? beforeMessageId,
    int? beforeAtMs,
    required int limit,
  }) => RuntimeRequestDto._(
    kind: 'query',
    name: 'conversation.page',
    payload: <String, Object?>{
      'conversationId': conversationId,
      'beforeMessageId': beforeMessageId,
      'beforeAtMs': beforeAtMs,
      'limit': limit,
    },
  );

  factory RuntimeRequestDto.conversationSearch(
    String conversationId, {
    required String query,
    required int limit,
  }) => RuntimeRequestDto._(
    kind: 'query',
    name: 'conversation.search',
    payload: <String, Object?>{
      'conversationId': conversationId,
      'query': query,
      'limit': limit,
    },
  );

  factory RuntimeRequestDto.notificationEvents(int afterCursor) =>
      RuntimeRequestDto._(
        kind: 'query',
        name: 'notifications.poll',
        payload: <String, Object?>{'afterCursor': afterCursor},
      );

  static const RuntimeRequestDto diagnostics = RuntimeRequestDto._(
    kind: 'query',
    name: 'diagnostics.get',
    payload: <String, Object?>{},
  );

  static const RuntimeRequestDto snapshot = RuntimeRequestDto._(
    kind: 'query',
    name: 'snapshot.get',
    payload: <String, Object?>{},
  );

  final String kind;
  final String name;
  final Map<String, Object?> payload;

  /// Contract timeout for a single ABI wait. The native actor independently
  /// bounds mailbox admission for network work to two seconds.
  int get timeoutMs => kind == 'query' ? 5000 : 10000;

  static RuntimeRequestDto? command(BridgeCommandDto command) {
    if (command is RefreshSnapshotCommandDto) return snapshot;
    if (command is UpdateProfileCommandDto) {
      return _command('profile.set', <String, Object?>{
        'displayName': command.displayName,
      });
    }
    if (command is CreatePairingCommandDto) {
      return _command('pairing.create', const <String, Object?>{});
    }
    if (command is JoinPairingCommandDto) {
      return _command('pairing.join', <String, Object?>{
        'code': command.code,
        if (command.ticket != null) 'ticket': command.ticket,
      });
    }
    if (command is ApprovePairingCommandDto) {
      return _session('pairing.approve', command.sessionIdHex);
    }
    if (command is RejectPairingCommandDto) {
      return _session('pairing.reject', command.sessionIdHex);
    }
    if (command is CancelPairingCommandDto) {
      return _session('pairing.cancel', command.sessionIdHex);
    }
    if (command is RenameContactCommandDto) {
      return _command('contact.rename', <String, Object?>{
        'contactIdHex': command.contactIdHex,
        'displayName': command.displayName,
      });
    }
    if (command is VerifyContactCommandDto) {
      return _contact('contact.verify', command.contactIdHex);
    }
    if (command is ResetContactVerificationCommandDto) {
      return _contact('contact.verification.reset', command.contactIdHex);
    }
    if (command is BlockContactCommandDto) {
      return _contact('contact.block', command.contactIdHex);
    }
    if (command is UnblockContactCommandDto) {
      return _contact('contact.unblock', command.contactIdHex);
    }
    if (command is RemoveContactCommandDto) {
      return _contact('contact.remove', command.contactIdHex);
    }
    if (command is StartConversationCommandDto) {
      return _contact('conversation.start', command.contactIdHex);
    }
    if (command is ClearConversationHistoryCommandDto) {
      return _command('conversation.clear', <String, Object?>{
        'conversationIdHex': command.conversationIdHex,
      });
    }
    if (command is QueueMessageCommandDto) {
      return _command('message.send', <String, Object?>{
        'conversationIdHex': command.conversationIdHex,
        'body': command.body,
        'replyToMessageIdHex': command.replyToMessageId,
      });
    }
    if (command is RetryMessageCommandDto) {
      return _command('message.retry', <String, Object?>{
        'messageIdHex': command.messageIdHex,
      });
    }
    if (command is MarkConversationReadCommandDto) {
      return _command('conversation.read', <String, Object?>{
        'conversationIdHex': command.conversationIdHex,
      });
    }
    if (command is QueueAttachmentCommandDto) {
      return _command('attachment.queue', <String, Object?>{
        'conversationIdHex': command.conversationIdHex,
        'sourcePath': command.sourcePath,
        'name': command.name,
        'mediaType': command.mediaType,
        'size': command.size,
      });
    }
    if (command is RetryAttachmentCommandDto) {
      return _command('attachment.retry', <String, Object?>{
        'attachmentIdHex': command.attachmentIdHex,
      });
    }
    if (command is CancelAttachmentCommandDto) {
      return _command('attachment.cancel', <String, Object?>{
        'attachmentIdHex': command.attachmentIdHex,
      });
    }
    if (command is ExportAttachmentCommandDto) {
      return _command('attachment.export', <String, Object?>{
        'attachmentIdHex': command.attachmentIdHex,
        'destinationPath': command.destinationPath,
      });
    }
    if (command is SetNotificationsCommandDto) {
      return _command('notifications.set', <String, Object?>{
        'enabled': command.enabled,
      });
    }
    if (command is AcknowledgeNewContactsCommandDto) {
      return _command('contacts.acknowledge_new', const <String, Object?>{});
    }
    return null;
  }

  static RuntimeRequestDto _command(
    String name,
    Map<String, Object?> payload,
  ) => RuntimeRequestDto._(kind: 'command', name: name, payload: payload);

  static RuntimeRequestDto _session(String name, String sessionIdHex) =>
      _command(name, <String, Object?>{'sessionIdHex': sessionIdHex});

  static RuntimeRequestDto _contact(String name, String contactIdHex) =>
      _command(name, <String, Object?>{'contactIdHex': contactIdHex});

  String encode(String requestId) => jsonEncode(<String, Object?>{
    'schema': 1,
    'requestId': requestId,
    'kind': kind,
    'name': name,
    'payload': payload,
  });
}
