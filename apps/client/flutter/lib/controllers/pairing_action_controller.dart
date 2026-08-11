import 'package:flutter/foundation.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/bridge_error_presenter.dart';

enum PairingAction { approve, reject, cancel }

extension PairingActionCommand on PairingAction {
  BridgeCommandDto command(String sessionId) => switch (this) {
    PairingAction.approve => ApprovePairingCommandDto(sessionIdHex: sessionId),
    PairingAction.reject => RejectPairingCommandDto(sessionIdHex: sessionId),
    PairingAction.cancel => CancelPairingCommandDto(sessionIdHex: sessionId),
  };
}

/// Owns the single-flight and error policy shared by pairing decision modals.
class PairingActionController extends ChangeNotifier {
  PairingActionController(this._gateway);

  final EngineGateway _gateway;
  bool _disposed = false;
  bool _busy = false;
  String? _error;

  bool get busy => _busy;
  String? get error => _error;

  Future<bool> run(PairingAction action, String sessionId) async {
    if (_busy || _disposed) return false;
    _busy = true;
    _error = null;
    notifyListeners();
    final result = await _gateway.execute(action.command(sessionId));
    if (_disposed) return result.ok;
    _busy = false;
    if (!result.ok) {
      _error = BridgeErrorPresenter.message(
        result,
        fallback: 'Pairing operation failed',
      );
    }
    notifyListeners();
    return result.ok;
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
