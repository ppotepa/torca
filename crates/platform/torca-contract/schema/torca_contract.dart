// GENERATED FILE. DO NOT EDIT.
// Generated from: crates/platform/torca-contract/schema/torca_contract.json

import 'dart:convert';

const int torcaContractVersion = __TORCA_CONTRACT_VERSION__;
const int torcaNativeAbiVersion = 1;

class ContractDecodeException extends FormatException {
  const ContractDecodeException(super.message);
}

String _requiredString(Map<String, dynamic> value, String key) {
  final field = value[key];
  if (field is String) return field;
  throw ContractDecodeException('Missing or invalid contract field: $key');
}

int _integer(Object? value, [int fallback = 0]) =>
    value is num ? value.toInt() : fallback;

List<Map<String, dynamic>> _objects(Object? value) => value is List
    ? value.whereType<Map<String, dynamic>>().toList(growable: false)
    : const <Map<String, dynamic>>[];

class BridgeResultDto {
  const BridgeResultDto({
    required this.ok,
    required this.kind,
    this.error,
    String? errorCode,
    this.messageKey,
    this.diagnosticId,
    this.retryable = false,
    this.resourceId,
    this.inviteUri,
  }) : _wireErrorCode = errorCode;
  final bool ok;
  final String kind;
  final String? error;
  final String? messageKey;
  final String? diagnosticId;
  final bool retryable;
  final String? resourceId;
  final String? inviteUri;
  final String? _wireErrorCode;
  String? get errorCode =>
      _wireErrorCode ?? (kind.startsWith('error:') ? kind.substring(6) : null);
}

class IdentityDto {
  const IdentityDto({this.displayName, this.fingerprint});
  factory IdentityDto.fromJson(Map<String, dynamic> value) => IdentityDto(
    displayName: value['displayName'] as String?,
    fingerprint: value['fingerprint'] as String?,
  );
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
  factory BootstrapStepDto.fromJson(Map<String, dynamic> value) =>
      BootstrapStepDto(
        id: _requiredString(value, 'id'),
        state: _requiredString(value, 'state'),
        code: value['code'] as String?,
        progress: _integer(value['progress']),
        attempt: _integer(value['attempt']),
        startedAtMs: (value['startedAtMs'] as num?)?.toInt(),
        lastProgressAtMs: (value['lastProgressAtMs'] as num?)?.toInt(),
        retryAtMs: (value['retryAtMs'] as num?)?.toInt(),
      );
  final String id, state;
  final String? code;
  final int progress, attempt;
  final int? startedAtMs, lastProgressAtMs, retryAtMs;

  BootstrapStepState get typedState => switch (state) {
    'pending' => BootstrapStepState.pending,
    'running' => BootstrapStepState.running,
    'verifying' => BootstrapStepState.verifying,
    'ready' => BootstrapStepState.ready,
    'degraded' => BootstrapStepState.degraded,
    'failed' => BootstrapStepState.failed,
    'blocked' => BootstrapStepState.blocked,
    _ => BootstrapStepState.unknown,
  };
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

enum BootstrapStepState {
  pending,
  running,
  verifying,
  ready,
  degraded,
  failed,
  blocked,
  unknown,
}

enum BootstrapPhase {
  idle,
  starting,
  readyForProfile,
  ready,
  degraded,
  failed,
  unknown,
}

enum TransportState {
  stopped,
  starting,
  checking,
  healthy,
  ready,
  degraded,
  unreachable,
  failed,
  disabled,
  inactive,
  disconnected,
  connecting,
  handshaking,
  reconnecting,
  unknown,
}

enum PeerHealthQuality { excellent, good, fair, poor, unknown }

enum ContactStatus { active, blocked, removed, unknown }

enum PresenceState { online, offline, unknown }

enum VerificationStatus { verified, unverified, identityChanged, unknown }

enum ConversationStatus { active, archived, unknown }

enum MessageDirection { outbound, inbound, unknown }

enum MessageStatus {
  queued,
  sending,
  sent,
  delivered,
  read,
  failed,
  cancelled,
  unknown,
}

enum AttachmentDirection { outbound, inbound, unknown }

enum AttachmentStatus {
  prepared,
  encrypting,
  queued,
  transferring,
  available,
  failed,
  cancelled,
  unknown,
}

enum PendingOperationState { queued, retrying, unknown }

enum PendingOperationDependency {
  torOnionAndRelay,
  relay,
  runtime,
  network,
  unknown,
}

// Generated wire adapters. Keep protocol spellings in one place so a Dart
// enum rename cannot silently change the external contract.
extension PairingRoleWire on PairingRole {
  String get wireValue => switch (this) {
    PairingRole.creator => 'creator',
    PairingRole.joiner => 'joiner',
    PairingRole.unknown => 'unknown',
  };
}
PairingRole pairingRoleFromWire(String value) => switch (value.toLowerCase()) {
  'creator' => PairingRole.creator,
  'joiner' => PairingRole.joiner,
  _ => PairingRole.unknown,
};

extension PairingStateWire on PairingState {
  String get wireValue => switch (this) {
    PairingState.open => 'open',
    PairingState.peerJoined => 'peer_joined',
    PairingState.awaitingApproval => 'awaiting_approval',
    PairingState.approved => 'approved',
    PairingState.completed => 'completed',
    PairingState.rejected => 'rejected',
    PairingState.cancelled => 'cancelled',
    PairingState.expired => 'expired',
    PairingState.unknown => 'unknown',
  };
}
PairingState pairingStateFromWire(String value) => switch (
  value.toLowerCase().replaceAll('-', '_'),
) {
  'open' => PairingState.open,
  'peer_joined' => PairingState.peerJoined,
  'awaiting_approval' => PairingState.awaitingApproval,
  'approved' => PairingState.approved,
  'completed' => PairingState.completed,
  'rejected' => PairingState.rejected,
  'cancelled' => PairingState.cancelled,
  'expired' => PairingState.expired,
  _ => PairingState.unknown,
};

extension ContactStatusWire on ContactStatus {
  String get wireValue => switch (this) {
    ContactStatus.active => 'active',
    ContactStatus.blocked => 'blocked',
    ContactStatus.removed => 'removed',
    ContactStatus.unknown => 'unknown',
  };
}
ContactStatus contactStatusFromWire(String value) => switch (value.toLowerCase()) {
  'active' => ContactStatus.active,
  'blocked' => ContactStatus.blocked,
  'removed' => ContactStatus.removed,
  _ => ContactStatus.unknown,
};

extension MessageStatusWire on MessageStatus {
  String get wireValue => switch (this) {
    MessageStatus.queued => 'queued',
    MessageStatus.sending => 'sending',
    MessageStatus.sent => 'sent',
    MessageStatus.delivered => 'delivered',
    MessageStatus.read => 'read',
    MessageStatus.failed => 'failed',
    MessageStatus.cancelled => 'cancelled',
    MessageStatus.unknown => 'unknown',
  };
}
MessageStatus messageStatusFromWire(String value) => switch (value.toLowerCase()) {
  'queued' => MessageStatus.queued,
  'sending' => MessageStatus.sending,
  'sent' => MessageStatus.sent,
  'delivered' => MessageStatus.delivered,
  'read' => MessageStatus.read,
  'failed' => MessageStatus.failed,
  'cancelled' => MessageStatus.cancelled,
  _ => MessageStatus.unknown,
};

extension AttachmentStatusWire on AttachmentStatus {
  String get wireValue => switch (this) {
    AttachmentStatus.prepared => 'prepared',
    AttachmentStatus.encrypting => 'encrypting',
    AttachmentStatus.queued => 'queued',
    AttachmentStatus.transferring => 'transferring',
    AttachmentStatus.available => 'available',
    AttachmentStatus.failed => 'failed',
    AttachmentStatus.cancelled => 'cancelled',
    AttachmentStatus.unknown => 'unknown',
  };
}
AttachmentStatus attachmentStatusFromWire(String value) => switch (value.toLowerCase()) {
  'prepared' => AttachmentStatus.prepared,
  'encrypting' => AttachmentStatus.encrypting,
  'queued' => AttachmentStatus.queued,
  'transferring' => AttachmentStatus.transferring,
  'available' => AttachmentStatus.available,
  'failed' => AttachmentStatus.failed,
  'cancelled' => AttachmentStatus.cancelled,
  _ => AttachmentStatus.unknown,
};

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
  factory PairingDto.fromJson(Map<String, dynamic> value) => PairingDto(
    id: _requiredString(value, 'id'),
    code: _requiredString(value, 'code'),
    inviteUri: value['inviteUri'] as String? ?? '',
    role: _requiredString(value, 'role'),
    state: _requiredString(value, 'state'),
    expiresAtMs: _integer(value['expiresAtMs']),
    localApproved: value['localApproved'] as bool? ?? false,
    remoteApproved: value['remoteApproved'] as bool? ?? false,
    remoteIdentityId: value['remoteIdentityId'] as String?,
    remoteDisplayName: value['remoteDisplayName'] as String?,
    remoteFingerprint: value['remoteFingerprint'] as String?,
  );
  final String id, code, inviteUri, role, state;
  final int expiresAtMs;
  final bool localApproved, remoteApproved;
  final String? remoteIdentityId, remoteDisplayName, remoteFingerprint;

  PairingRole get typedRole => pairingRoleFromWire(role);

  PairingState get typedState => pairingStateFromWire(state);
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
  factory PeerHealthDto.fromJson(Map<String, dynamic> value) => PeerHealthDto(
    state: value['state'] as String? ?? 'disconnected',
    quality: value['quality'] as String? ?? 'unknown',
    rttMs: (value['rttMs'] as num?)?.toInt(),
    lastSuccessAtMs: (value['lastSuccessAtMs'] as num?)?.toInt(),
    consecutiveFailures: _integer(value['consecutiveFailures']),
    reconnectAttempt: _integer(value['reconnectAttempt']),
    lastActivityAtMs: (value['lastActivityAtMs'] as num?)?.toInt(),
    activitySequence: _integer(value['activitySequence']),
  );
  final String state, quality;
  final int? rttMs, lastSuccessAtMs, lastActivityAtMs;
  final int consecutiveFailures, reconnectAttempt, activitySequence;

  TransportState get typedState => _transportState(state);
  PeerHealthQuality get typedQuality => switch (quality) {
    'excellent' => PeerHealthQuality.excellent,
    'good' => PeerHealthQuality.good,
    'fair' => PeerHealthQuality.fair,
    'poor' => PeerHealthQuality.poor,
    _ => PeerHealthQuality.unknown,
  };
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
  factory TransportIndicatorDto.fromJson(
    Map<String, dynamic> value, {
    String fallbackState = 'unknown',
  }) => TransportIndicatorDto(
    state: value['state'] as String? ?? fallbackState,
    code: value['code'] as String? ?? 'UNAVAILABLE',
    latencyMs: (value['latencyMs'] as num?)?.toInt(),
    lastActivityAtMs: (value['lastActivityAtMs'] as num?)?.toInt(),
    activitySequence: _integer(value['activitySequence']),
    txSequence: _integer(value['txSequence']),
    rxSequence: _integer(value['rxSequence']),
    inFlight: _integer(value['inFlight']),
    queued: _integer(value['queued']),
  );
  final String state, code;
  final int? latencyMs, lastActivityAtMs;
  final int activitySequence, txSequence, rxSequence, inFlight, queued;

  TransportState get typedState => _transportState(state);
  bool get isUsable =>
      typedState == TransportState.healthy ||
      typedState == TransportState.ready;
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
  factory TransportStatusDto.fromJson(Map<String, dynamic> value) {
    final tor = value['tor'];
    final relay = value['relay'];
    final peer = value['peer'];
    final relayInfo = value['relayInfo'];
    return TransportStatusDto(
      tor: tor is Map<String, dynamic>
          ? TransportIndicatorDto.fromJson(tor, fallbackState: 'stopped')
          : const TransportIndicatorDto(state: 'stopped'),
      relay: relay is Map<String, dynamic>
          ? TransportIndicatorDto.fromJson(relay)
          : const TransportIndicatorDto(),
      peer: peer is Map<String, dynamic>
          ? TransportIndicatorDto.fromJson(peer, fallbackState: 'disconnected')
          : const TransportIndicatorDto(state: 'disconnected'),
      peersReady: _integer(value['peersReady']),
      peersTotal: _integer(value['peersTotal']),
      relayInfo: relayInfo is Map<String, dynamic>
          ? RelayInfoDto.fromJson(relayInfo)
          : null,
    );
  }
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
  factory RelayInfoDto.fromJson(Map<String, dynamic> value) => RelayInfoDto(
    productVersion: _requiredString(value, 'productVersion'),
    buildId: _requiredString(value, 'buildId'),
    sourceCommit: _requiredString(value, 'sourceCommit'),
    protocolVersion: _integer(value['protocolVersion']),
  );
  final String productVersion, buildId, sourceCommit;
  final int protocolVersion;
}

class NavigationBadgesDto {
  const NavigationBadgesDto({
    this.unreadMessages = 0,
    this.newContacts = 0,
    this.pairingAttention = 0,
  });
  factory NavigationBadgesDto.fromJson(Map<String, dynamic> value) =>
      NavigationBadgesDto(
        unreadMessages: _integer(value['unreadMessages']),
        newContacts: _integer(value['newContacts']),
        pairingAttention: _integer(value['pairingAttention']),
      );
  final int unreadMessages, newContacts, pairingAttention;
}

class ContactDto {
  const ContactDto({
    required this.id,
    required this.displayName,
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
  factory ContactDto.fromJson(Map<String, dynamic> value) {
    final peerHealth = value['peerHealth'];
    return ContactDto(
      id: _requiredString(value, 'id'),
      displayName: _requiredString(value, 'displayName'),
      onionAddress: _requiredString(value, 'onionAddress'),
      status: _requiredString(value, 'status'),
      connectionState: _requiredString(value, 'connectionState'),
      presenceState: value['presenceState'] as String? ?? 'unknown',
      lastSeenAtMs: (value['lastSeenAtMs'] as num?)?.toInt(),
      safetyNumber: value['safetyNumber'] as String?,
      peerHealth: peerHealth is Map<String, dynamic>
          ? PeerHealthDto.fromJson(peerHealth)
          : const PeerHealthDto(),
      verificationStatus:
          value['verificationStatus'] as String? ?? 'unverified',
      verifiedAtMs: (value['verifiedAtMs'] as num?)?.toInt(),
    );
  }
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

  ContactStatus get typedStatus => switch (status) {
    'active' => ContactStatus.active,
    'blocked' => ContactStatus.blocked,
    'removed' => ContactStatus.removed,
    _ => ContactStatus.unknown,
  };
  TransportState get typedConnectionState => _transportState(connectionState);
  PresenceState get typedPresenceState => switch (presenceState) {
    'online' => PresenceState.online,
    'offline' => PresenceState.offline,
    _ => PresenceState.unknown,
  };
  VerificationStatus get typedVerificationStatus =>
      switch (verificationStatus) {
        'verified' => VerificationStatus.verified,
        'unverified' => VerificationStatus.unverified,
        'identity_changed' => VerificationStatus.identityChanged,
        _ => VerificationStatus.unknown,
      };
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
  factory ConversationDto.fromJson(Map<String, dynamic> value) =>
      ConversationDto(
        id: _requiredString(value, 'id'),
        contactId: _requiredString(value, 'contactId'),
        status: _requiredString(value, 'status'),
        unreadCount: _integer(value['unreadCount']),
        lastActivityAtMs: _integer(value['lastActivityAtMs']),
        lastMessageBody: value['lastMessageBody'] as String?,
        lastMessageDirection: value['lastMessageDirection'] as String?,
        lastMessageStatus: value['lastMessageStatus'] as String?,
      );
  final String id, contactId, status;
  final int unreadCount, lastActivityAtMs;
  final String? lastMessageBody, lastMessageDirection, lastMessageStatus;

  ConversationStatus get typedStatus => switch (status) {
    'active' => ConversationStatus.active,
    'archived' => ConversationStatus.archived,
    _ => ConversationStatus.unknown,
  };
  MessageDirection? get typedLastMessageDirection =>
      lastMessageDirection == null
      ? null
      : _messageDirection(lastMessageDirection!);
  MessageStatus? get typedLastMessageStatus =>
      lastMessageStatus == null ? null : _messageStatus(lastMessageStatus!);
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
  factory MessageDto.fromJson(Map<String, dynamic> value) => MessageDto(
    id: _requiredString(value, 'id'),
    conversationId: _requiredString(value, 'conversationId'),
    body: _requiredString(value, 'body'),
    direction: _requiredString(value, 'direction'),
    status: _requiredString(value, 'status'),
    replyToMessageId: value['replyToMessageId'] as String?,
    createdAtMs: _integer(value['createdAtMs']),
    updatedAtMs: _integer(value['updatedAtMs']),
    sentAtMs: (value['sentAtMs'] as num?)?.toInt(),
    deliveredAtMs: (value['deliveredAtMs'] as num?)?.toInt(),
    readAtMs: (value['readAtMs'] as num?)?.toInt(),
    attemptCount: _integer(value['attemptCount']),
  );
  final String id, conversationId, body, direction, status;
  final String? replyToMessageId;
  final int createdAtMs, updatedAtMs, attemptCount;
  final int? sentAtMs, deliveredAtMs, readAtMs;

  MessageDirection get typedDirection => _messageDirection(direction);
  MessageStatus get typedStatus => _messageStatus(status);
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
    this.lastErrorCode,
  });
  factory AttachmentDto.fromJson(Map<String, dynamic> value) => AttachmentDto(
    id: _requiredString(value, 'id'),
    messageId: _requiredString(value, 'messageId'),
    name: _requiredString(value, 'name'),
    mediaType: _requiredString(value, 'mediaType'),
    size: _integer(value['size']),
    status: _requiredString(value, 'status'),
    offset: _integer(value['offset']),
    attemptCount: _integer(value['attemptCount']),
    updatedAtMs: _integer(value['updatedAtMs']),
    direction: value['direction'] as String? ?? 'outbound',
    lastErrorCode: value['lastErrorCode'] as String?,
  );
  final String id, messageId, name, mediaType, status;
  final int size, offset, attemptCount, updatedAtMs;
  final String direction;
  final String? lastErrorCode;

  AttachmentDirection get typedDirection => switch (direction) {
    'outbound' => AttachmentDirection.outbound,
    'inbound' => AttachmentDirection.inbound,
    _ => AttachmentDirection.unknown,
  };
  AttachmentStatus get typedStatus => switch (status) {
    'prepared' => AttachmentStatus.prepared,
    'encrypting' => AttachmentStatus.encrypting,
    'queued' => AttachmentStatus.queued,
    'transferring' => AttachmentStatus.transferring,
    'available' => AttachmentStatus.available,
    'failed' => AttachmentStatus.failed,
    'cancelled' => AttachmentStatus.cancelled,
    _ => AttachmentStatus.unknown,
  };
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
  factory PendingOperationDto.fromJson(Map<String, dynamic> value) =>
      PendingOperationDto(
        id: _requiredString(value, 'id'),
        resourceId: _requiredString(value, 'resourceId'),
        kind: _requiredString(value, 'kind'),
        state: _requiredString(value, 'state'),
        dependency: _requiredString(value, 'dependency'),
        attempts: _integer(value['attempts']),
        nextAttemptAtMs: _integer(value['nextAttemptAtMs']),
        createdAtMs: _integer(value['createdAtMs']),
        lastError: value['lastError'] as String?,
      );
  final String id, resourceId, kind, state, dependency;
  final int attempts, nextAttemptAtMs, createdAtMs;
  final String? lastError;

  PendingOperationState get typedState => switch (state) {
    'queued' => PendingOperationState.queued,
    'retrying' => PendingOperationState.retrying,
    _ => PendingOperationState.unknown,
  };
  PendingOperationDependency get typedDependency => switch (dependency) {
    'tor_onion_and_relay' => PendingOperationDependency.torOnionAndRelay,
    'relay' => PendingOperationDependency.relay,
    'runtime' => PendingOperationDependency.runtime,
    'network' => PendingOperationDependency.network,
    _ => PendingOperationDependency.unknown,
  };
}

class AppSnapshotDto {
  const AppSnapshotDto({
    this.runtimeId = '',
    this.revision = 0,
    this.notificationCursor = 0,
    this.notificationsEnabled = true,
    this.readReceiptsEnabled = true,
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
  factory AppSnapshotDto.fromJson(Map<String, dynamic> value) {
    final identity = value['identity'];
    final transport = value['transport'];
    final navigationBadges = value['navigationBadges'];
    return AppSnapshotDto(
      runtimeId: value['runtimeId'] as String? ?? '',
      revision: _integer(value['revision']),
      notificationCursor: _integer(value['notificationCursor']),
      notificationsEnabled: value['notificationsEnabled'] as bool? ?? true,
      readReceiptsEnabled: value['readReceiptsEnabled'] as bool? ?? true,
      identity: identity is Map<String, dynamic>
          ? IdentityDto.fromJson(identity)
          : null,
      torState: value['torState'] as String? ?? 'stopped',
      transport: transport is Map<String, dynamic>
          ? TransportStatusDto.fromJson(transport)
          : const TransportStatusDto(),
      navigationBadges: navigationBadges is Map<String, dynamic>
          ? NavigationBadgesDto.fromJson(navigationBadges)
          : const NavigationBadgesDto(),
      onionAddress: value['onionAddress'] as String?,
      pairings: _objects(
        value['pairings'],
      ).map(PairingDto.fromJson).toList(growable: false),
      contacts: _objects(
        value['contacts'],
      ).map(ContactDto.fromJson).toList(growable: false),
      conversations: _objects(
        value['conversations'],
      ).map(ConversationDto.fromJson).toList(growable: false),
      messages: _objects(
        value['messages'],
      ).map(MessageDto.fromJson).toList(growable: false),
      attachments: _objects(
        value['attachments'],
      ).map(AttachmentDto.fromJson).toList(growable: false),
      pendingOperations: _objects(
        value['pendingOperations'],
      ).map(PendingOperationDto.fromJson).toList(growable: false),
      bootstrapPhase: value['bootstrapPhase'] as String? ?? 'failed',
      bootstrapSteps: _objects(
        value['bootstrapSteps'],
      ).map(BootstrapStepDto.fromJson).toList(growable: false),
    );
  }
  final String runtimeId;
  final int revision, notificationCursor;
  final bool notificationsEnabled, readReceiptsEnabled;
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

  BootstrapPhase get typedBootstrapPhase => switch (bootstrapPhase) {
    'idle' => BootstrapPhase.idle,
    'starting' => BootstrapPhase.starting,
    'ready_for_profile' => BootstrapPhase.readyForProfile,
    'ready' => BootstrapPhase.ready,
    'degraded' => BootstrapPhase.degraded,
    'failed' => BootstrapPhase.failed,
    _ => BootstrapPhase.unknown,
  };
}

TransportState _transportState(String value) => switch (value) {
  'stopped' => TransportState.stopped,
  'starting' => TransportState.starting,
  'checking' => TransportState.checking,
  'healthy' => TransportState.healthy,
  'ready' => TransportState.ready,
  'degraded' => TransportState.degraded,
  'unreachable' => TransportState.unreachable,
  'failed' => TransportState.failed,
  'disabled' => TransportState.disabled,
  'inactive' => TransportState.inactive,
  'disconnected' => TransportState.disconnected,
  'connecting' => TransportState.connecting,
  'handshaking' => TransportState.handshaking,
  'reconnecting' => TransportState.reconnecting,
  _ => TransportState.unknown,
};

MessageDirection _messageDirection(String value) => switch (value) {
  'outbound' => MessageDirection.outbound,
  'inbound' => MessageDirection.inbound,
  _ => MessageDirection.unknown,
};

MessageStatus _messageStatus(String value) => switch (value) {
  'queued' => MessageStatus.queued,
  'sending' => MessageStatus.sending,
  'sent' => MessageStatus.sent,
  'delivered' => MessageStatus.delivered,
  'read' => MessageStatus.read,
  'failed' => MessageStatus.failed,
  'cancelled' => MessageStatus.cancelled,
  _ => MessageStatus.unknown,
};

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
    this.previewSourcePath,
    required this.name,
    required this.mediaType,
    required this.size,
  });
  final String conversationIdHex, sourcePath, name, mediaType;
  final String? previewSourcePath;
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

class ExportAttachmentPreviewCommandDto extends BridgeCommandDto {
  const ExportAttachmentPreviewCommandDto({
    required this.attachmentIdHex,
    required this.destinationPath,
  });
  final String attachmentIdHex, destinationPath;
}

class SetNotificationsCommandDto extends BridgeCommandDto {
  const SetNotificationsCommandDto({required this.enabled});
  final bool enabled;
}

class SetReadReceiptsEnabledCommandDto extends BridgeCommandDto {
  const SetReadReceiptsEnabledCommandDto({required this.enabled});
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

  factory RuntimeRequestDto.runtimePoll(int afterCursor) => RuntimeRequestDto._(
    kind: 'query',
    name: 'runtime.poll',
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
        'previewSourcePath': command.previewSourcePath,
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
    if (command is ExportAttachmentPreviewCommandDto) {
      return _command('attachment.preview.export', <String, Object?>{
        'attachmentIdHex': command.attachmentIdHex,
        'destinationPath': command.destinationPath,
      });
    }
    if (command is SetNotificationsCommandDto) {
      return _command('notifications.set', <String, Object?>{
        'enabled': command.enabled,
      });
    }
    if (command is SetReadReceiptsEnabledCommandDto) {
      return _command('privacy.read_receipts.set', <String, Object?>{
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
