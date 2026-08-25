import 'package:flutter/widgets.dart';

import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';

abstract final class BridgeErrorPresenter {
  /// Returns a localized user-facing message without exposing the native
  /// diagnostic text. The non-localized helper below remains for controllers
  /// that do not own a BuildContext (they should pass a localized fallback
  /// when presenting the result).
  static String localized(
    BuildContext context,
    BridgeResultDto result, {
    String? fallback,
  }) {
    if (result.ok) return '';
    final strings = TorcaStrings.of(context);
    final code = (result.errorCode ?? result.messageKey ?? '')
        .trim()
        .toLowerCase()
        .replaceAll('.', '_');
    final message = switch (code) {
      'relay_not_ready' => strings.relayNotReady,
      'relay_degraded' => strings.relayDegraded,
      'profile_not_ready' => strings.profileNotReady,
      'identity_changed' => strings.identityChanged,
      'pairing_expired' => strings.pairingExpired,
      'pairing_bootstrap_missing' => strings.pairingBootstrapRequired,
      'pairing_provider_mismatch' => strings.pairingProviderMismatch,
      'pairing_bootstrap_invalid' => strings.invalidInput,
      'already_exists' => strings.itemAlreadyExists,
      'not_found' => strings.itemNotFound,
      'invalid_input' => strings.invalidInput,
      'storage_failure' => strings.storageFailure,
      'attachment_failure' => strings.attachmentOperationFailed,
      'attachment_ack_timeout' ||
      'communication_attachment_ack_timeout' => strings.attachmentAckTimeout,
      'attachment_peer_unavailable' ||
      'communication_attachment_peer_unavailable' =>
        strings.attachmentPeerUnavailable,
      'attachment_integrity_failed' ||
      'communication_attachment_integrity_failed' =>
        strings.attachmentIntegrityFailed,
      'network_unavailable' => strings.networkUnavailable,
      'runtime_unavailable' => strings.runtimeUnavailable,
      'contract_decode_failed' => strings.contractDecodeFailed,
      _ => fallback ?? strings.operationFailed,
    };
    return message;
  }

  static String message(
    BridgeResultDto result, {
    String fallback = 'The operation could not be completed.',
    String provider = 'tor',
  }) {
    if (result.ok) return '';
    final code = (result.errorCode ?? result.messageKey ?? '')
        .trim()
        .toLowerCase()
        .replaceAll('.', '_');
    final typed = switch (code) {
      'relay_not_ready' =>
        'Pairing is unavailable until the secure relay is ready.',
      'relay_degraded' =>
        'Pairing is temporarily unavailable while the relay reconnects.',
      'profile_not_ready' =>
        'The secure runtime is not ready for profile setup.',
      'identity_changed' =>
        'Contact identity changed. Verify the new Safety Number before sending.',
      'pairing_expired' => 'The pairing invitation has expired.',
      'pairing_bootstrap_missing' =>
        'This provider requires the QR code or the full invitation link.',
      'pairing_provider_mismatch' =>
        'This invitation belongs to a different communication provider.',
      'pairing_bootstrap_invalid' => 'The invitation bootstrap is invalid.',
      'already_exists' => 'This item already exists.',
      'not_found' => 'The requested item is no longer available.',
      'invalid_input' => 'The supplied value is not valid.',
      'storage_failure' =>
        'Encrypted local storage could not complete the operation.',
      'attachment_failure' =>
        'The attachment operation could not be completed.',
      'attachment_ack_timeout' =>
        'The peer did not acknowledge the attachment in time. It will be retried.',
      'communication_attachment_ack_timeout' =>
        'The peer did not acknowledge the attachment in time. It will be retried.',
      'attachment_peer_unavailable' =>
        'The peer connection is unavailable. The attachment will be retried.',
      'communication_attachment_peer_unavailable' =>
        'The peer connection is unavailable. The attachment will be retried.',
      'attachment_integrity_failed' =>
        'The attachment failed its integrity check.',
      'communication_attachment_integrity_failed' =>
        'The attachment failed its integrity check.',
      'attachment_dependency_missing' =>
        'The attachment is waiting for its conversation message.',
      'communication_attachment_dependency_missing' =>
        'The attachment is waiting for its conversation message.',
      'communication_attachment_storage_failed' =>
        'The attachment could not be stored locally.',
      'communication_attachment_protocol_failed' =>
        'The attachment protocol failed. Please retry the transfer.',
      'attachment_message_pending' =>
        'The attachment is waiting for the conversation message to arrive.',
      'communication_attachment_unavailable' =>
        'The attachment transfer is temporarily unavailable.',
      'network_unavailable' => _networkUnavailableMessage(provider),
      'runtime_unavailable' =>
        'The secure Torca runtime is currently unavailable.',
      'contract_decode_failed' =>
        'The installed client and native runtime use incompatible data. Rebuild and redeploy both clients.',
      'operation_conflict' =>
        'The operation is not valid in the current state.',
      _ => null,
    };
    if (typed != null) return typed;
    // `error` is diagnostic transport data, not a user-facing API. Showing
    // it here leaked backend wording and made the UI depend on error text.
    // New codes must be mapped above (or localized through messageKey).
    return fallback;
  }

  static String _networkUnavailableMessage(String provider) {
    final normalized = provider.trim().toLowerCase();
    if (normalized.isEmpty || normalized == 'tor') {
      // Keep the legacy wording for callers that do not yet have a provider
      // snapshot. It is also the stable message used by older clients.
      return 'The secure Tor peer connection is currently unavailable.';
    }
    final label = switch (normalized) {
      'iroh' => 'Iroh',
      'webrtc' => 'WebRTC',
      _ => provider.trim(),
    };
    return 'The $label peer connection is currently unavailable.';
  }
}
