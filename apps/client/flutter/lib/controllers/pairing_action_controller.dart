import 'package:flutter/widgets.dart';
import 'package:torca_l10n/torca_l10n.dart';

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
  BridgeResultDto? _failure;

  bool get busy => _busy;
  String? error(BuildContext context) => _failure == null
      ? null
      : BridgeErrorPresenter.localized(
          context,
          _failure!,
          fallback: context.strings.invitationOperationFailed,
        );

  Future<bool> run(PairingAction action, String sessionId) async {
    if (_busy || _disposed) return false;
    _busy = true;
    _failure = null;
    notifyListeners();
    final result = await _gateway.execute(action.command(sessionId));
    if (_disposed) return result.ok;
    _busy = false;
    if (!result.ok) {
      _failure = result;
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
