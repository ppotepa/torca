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
