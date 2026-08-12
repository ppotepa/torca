import '../generated/torca_contract.dart';

abstract final class BridgeErrorPresenter {
  static String message(
    BridgeResultDto result, {
    String fallback = 'The operation could not be completed.',
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
      'network_unavailable' =>
        'The secure Tor peer connection is currently unavailable.',
      'runtime_unavailable' =>
        'The secure Torca runtime is currently unavailable.',
      'operation_conflict' =>
        'The operation is not valid in the current state.',
      _ => null,
    };
    if (typed != null) return typed;
    final explicit = result.error?.trim();
    if (explicit != null && explicit.isNotEmpty && !explicit.contains('.')) {
      return explicit;
    }
    return fallback;
  }
}
