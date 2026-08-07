// GENERATED FILE. DO NOT EDIT.
// Source: crates/platform/torca-bridge/schema/torca_contract.dart

const int torcaContractVersion = 7;

class BridgeResultDto {
  const BridgeResultDto({required this.ok, required this.kind, this.error});
  final bool ok;
  final String kind;
  final String? error;
}

class IdentityDto {
  const IdentityDto({required this.displayName});
  final String displayName;
}

class PairingDto {
  const PairingDto({required this.id, required this.code, required this.role, required this.state, required this.expiresAtMs, required this.localApproved, required this.remoteApproved});
  final String id;
  final String code;
  final String role;
  final String state;
  final int expiresAtMs;
  final bool localApproved;
  final bool remoteApproved;
}

class ContactDto {
  const ContactDto({required this.id, required this.onionAddress, required this.status, required this.connectionState, this.safetyNumber});
  final String id;
  final String onionAddress;
  final String status;
  final String connectionState;
  final String? safetyNumber;
}

class ConversationDto {
  const ConversationDto({required this.id, required this.contactId, required this.status});
  final String id;
  final String contactId;
  final String status;
}

class MessageDto {
  const MessageDto({required this.id, required this.conversationId, required this.body, required this.direction, required this.status, this.replyToMessageId});
  final String id;
  final String conversationId;
  final String body;
  final String direction;
  final String status;
  final String? replyToMessageId;
}

class AttachmentDto {
  const AttachmentDto({required this.id, required this.messageId, required this.name, required this.mediaType, required this.size, required this.status, required this.offset});
  final String id;
  final String messageId;
  final String name;
  final String mediaType;
  final int size;
  final String status;
  final int offset;
}

class AppSnapshotDto {
  const AppSnapshotDto({this.identity, this.torState = 'stopped', this.onionAddress, this.pairings = const [], this.contacts = const [], this.conversations = const [], this.messages = const [], this.attachments = const []});
  final IdentityDto? identity;
  final String torState;
  final String? onionAddress;
  final List<PairingDto> pairings;
  final List<ContactDto> contacts;
  final List<ConversationDto> conversations;
  final List<MessageDto> messages;
  final List<AttachmentDto> attachments;
}

sealed class BridgeCommandDto { const BridgeCommandDto(); }
class CreateIdentityCommandDto extends BridgeCommandDto { const CreateIdentityCommandDto({required this.identityIdHex, required this.displayName, required this.atMs}); final String identityIdHex; final String displayName; final int atMs; }
class CreatePairingCommandDto extends BridgeCommandDto { const CreatePairingCommandDto({required this.sessionIdHex}); final String sessionIdHex; }
class JoinPairingCommandDto extends BridgeCommandDto { const JoinPairingCommandDto({required this.sessionIdHex, required this.code}); final String sessionIdHex; final String code; }
class ApprovePairingCommandDto extends BridgeCommandDto { const ApprovePairingCommandDto({required this.sessionIdHex}); final String sessionIdHex; }
class RejectPairingCommandDto extends BridgeCommandDto { const RejectPairingCommandDto({required this.sessionIdHex}); final String sessionIdHex; }
class CancelPairingCommandDto extends BridgeCommandDto { const CancelPairingCommandDto({required this.sessionIdHex}); final String sessionIdHex; }
class QueueMessageCommandDto extends BridgeCommandDto {
  const QueueMessageCommandDto({required this.messageIdHex, required this.conversationIdHex, required this.body, required this.atMs, this.replyToMessageId});
  final String messageIdHex; final String conversationIdHex; final String body; final int atMs; final String? replyToMessageId;
}
class RetryMessageCommandDto extends BridgeCommandDto { const RetryMessageCommandDto({required this.messageIdHex, required this.atMs}); final String messageIdHex; final int atMs; }
class MarkConversationReadCommandDto extends BridgeCommandDto { const MarkConversationReadCommandDto({required this.conversationIdHex}); final String conversationIdHex; }
class QueueAttachmentCommandDto extends BridgeCommandDto {
  const QueueAttachmentCommandDto({required this.attachmentIdHex, required this.messageIdHex, required this.conversationIdHex, required this.sourcePath, required this.name, required this.mediaType, required this.size});
  final String attachmentIdHex; final String messageIdHex; final String conversationIdHex; final String sourcePath; final String name; final String mediaType; final int size;
}
class RetryAttachmentCommandDto extends BridgeCommandDto { const RetryAttachmentCommandDto({required this.attachmentIdHex}); final String attachmentIdHex; }
class CancelAttachmentCommandDto extends BridgeCommandDto { const CancelAttachmentCommandDto({required this.attachmentIdHex}); final String attachmentIdHex; }
class RefreshSnapshotCommandDto extends BridgeCommandDto { const RefreshSnapshotCommandDto(); }
