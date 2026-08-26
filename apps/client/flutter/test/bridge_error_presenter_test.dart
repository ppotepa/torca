import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/widgets/bridge_error_presenter.dart';

void main() {
  test('typed error code is exposed from bridge kind', () {
    const result = BridgeResultDto(
      ok: false,
      kind: 'error:network_unavailable',
    );
    expect(result.errorCode, 'network_unavailable');
    expect(
      BridgeErrorPresenter.message(result),
      'The secure Tor peer connection is currently unavailable.',
    );
  });

  test('native sanitized message wins when present', () {
    const result = BridgeResultDto(
      ok: false,
      kind: 'error:operation_failed',
      error: 'The operation could not be completed.',
    );
    expect(
      BridgeErrorPresenter.message(result),
      'The operation could not be completed.',
    );
  });

  test('localization keys are never rendered as user-facing text', () {
    const result = BridgeResultDto(
      ok: false,
      kind: 'error:RELAY_NOT_READY',
      messageKey: 'relay.not_ready',
    );
    expect(
      BridgeErrorPresenter.message(result),
      'Pairing is unavailable until the secure relay is ready.',
    );
  });

  test('route refresh is presented as a retryable provider-neutral state', () {
    const result = BridgeResultDto(
      ok: false,
      kind: 'error:runtime.route_refresh_required',
    );
    expect(
      BridgeErrorPresenter.message(result),
      'The communication route is being refreshed. Try again shortly.',
    );
  });
}
