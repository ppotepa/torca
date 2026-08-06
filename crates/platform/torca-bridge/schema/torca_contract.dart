// GENERATED FILE. DO NOT EDIT.
// Source: crates/platform/torca-bridge/schema/torca_contract.dart

const int torcaContractVersion = 1;

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

class ContactDto {
  const ContactDto({
    required this.id,
    required this.onionAddress,
    required this.status,
  });
  final String id;
  final String onionAddress;
  final String status;
}

class ConversationDto {
  const ConversationDto({
    required this.id,
    required this.contactId,
    required this.status,
  });
  final String id;
  final String contactId;
  final String status;
}

class MessageDto {
  const MessageDto({
    required this.id,
    required this.conversationId,
    required this.body,
    required this.direction,
    required this.status,
  });
  final String id;
  final String conversationId;
  final String body;
  final String direction;
  final String status;
}

class AppSnapshotDto {
  const AppSnapshotDto({
    this.identity,
    this.contacts = const [],
    this.conversations = const [],
    this.messages = const [],
  });
  final IdentityDto? identity;
  final List<ContactDto> contacts;
  final List<ConversationDto> conversations;
  final List<MessageDto> messages;
}

sealed class BridgeCommandDto {
  const BridgeCommandDto();
}

class CreateIdentityCommandDto extends BridgeCommandDto {
  const CreateIdentityCommandDto({
    required this.identityIdHex,
    required this.displayName,
    required this.atMs,
  });
  final String identityIdHex;
  final String displayName;
  final int atMs;
}

class StartPairingCommandDto extends BridgeCommandDto {
  const StartPairingCommandDto({
    required this.sessionIdHex,
    required this.code,
    required this.expiresAtMs,
  });
  final String sessionIdHex;
  final String code;
  final int expiresAtMs;
}

class QueueMessageCommandDto extends BridgeCommandDto {
  const QueueMessageCommandDto({
    required this.messageIdHex,
    required this.conversationIdHex,
    required this.body,
    required this.atMs,
  });
  final String messageIdHex;
  final String conversationIdHex;
  final String body;
  final int atMs;
}

class RefreshSnapshotCommandDto extends BridgeCommandDto {
  const RefreshSnapshotCommandDto();
}
