import 'dart:collection';

/// Process-local ownership registry for pairing presentation.
///
/// A creator modal already contains the approval journey for its invitation.
/// The application-wide pairing observer must not put a second approval dialog
/// above it when the remote device joins.
class PairingModalRegistry {
  PairingModalRegistry._();

  static final PairingModalRegistry instance = PairingModalRegistry._();

  final Set<String> _ownerModalSessions = <String>{};

  UnmodifiableSetView<String> get ownerModalSessions =>
      UnmodifiableSetView<String>(_ownerModalSessions);

  bool owns(String sessionId) => _ownerModalSessions.contains(sessionId);

  void claim(String sessionId) {
    if (sessionId.isNotEmpty) _ownerModalSessions.add(sessionId);
  }

  void release(String? sessionId) {
    if (sessionId != null) _ownerModalSessions.remove(sessionId);
  }
}
