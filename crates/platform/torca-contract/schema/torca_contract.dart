// GENERATED FILE. DO NOT EDIT.
// Generated from: crates/platform/torca-contract/schema/torca_contract.json
const int torcaContractVersion = 12;
const int torcaNativeAbiVersion = 1;

class BridgeResultDto {
  const BridgeResultDto({required this.ok, required this.kind, this.error});
  final bool ok;
  final String kind;
  final String? error;
  String? get errorCode => kind.startsWith('error:') ? kind.substring(6) : null;
}

class IdentityDto {
  const IdentityDto({this.displayName, this.fingerprint});
  final String? displayName;
  final String? fingerprint;
}

class BootstrapStepDto {
  const BootstrapStepDto({required this.id, required this.state, this.code});
  final String id, state;
  final String? code;
}

class PairingDto {
  const PairingDto({
    required this.id,
    required this.code,
    required this.role,
    required this.state,
    required this.expiresAtMs,
    required this.localApproved,
    required this.remoteApproved,
  });
  final String id, code, role, state;
  final int expiresAtMs;
  final bool localApproved, remoteApproved;
}

class PeerHealthDto {
  const PeerHealthDto({
    this.state = 'disconnected',
    this.quality = 'unknown',
    this.rttMs,
    this.lastSuccessAtMs,
    this.consecutiveFailures = 0,
    this.reconnectAttempt = 0,
  });
  final String state, quality;
  final int? rttMs, lastSuccessAtMs;
  final int consecutiveFailures, reconnectAttempt;
}

class ContactDto {
  const ContactDto({
    required this.id,
    this.displayName = 'Contact',
    required this.onionAddress,
    required this.status,
    required this.connectionState,
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
      verificationStatus;
  final String? safetyNumber;
  final PeerHealthDto peerHealth;
  final int? verifiedAtMs;
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
    this.attemptCount = 0,
  });
  final String id, conversationId, body, direction, status;
  final String? replyToMessageId;
  final int createdAtMs, updatedAtMs, attemptCount;
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
  });
  final String id, messageId, name, mediaType, status;
  final int size, offset;
}

class AppSnapshotDto {
  const AppSnapshotDto({
    this.runtimeId = '',
    this.revision = 0,
    this.notificationCursor = 0,
    this.notificationsEnabled = true,
    this.identity,
    this.torState = 'stopped',
    this.onionAddress,
    this.pairings = const [],
    this.contacts = const [],
    this.conversations = const [],
    this.messages = const [],
    this.attachments = const [],
    this.bootstrapPhase = 'failed',
    this.bootstrapSteps = const [],
  });
  final String runtimeId;
  final int revision, notificationCursor;
  final bool notificationsEnabled;
  final IdentityDto? identity;
  final String torState;
  final String? onionAddress;
  final List<PairingDto> pairings;
  final List<ContactDto> contacts;
  final List<ConversationDto> conversations;
  final List<MessageDto> messages;
  final List<AttachmentDto> attachments;
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
  const JoinPairingCommandDto({required this.code});
  final String code;
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
  const MarkConversationReadCommandDto({
    required this.conversationIdHex,
    this.sendReceipt = true,
  });
  final String conversationIdHex;
  final bool sendReceipt;
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

class RefreshSnapshotCommandDto extends BridgeCommandDto {
  const RefreshSnapshotCommandDto();
}
