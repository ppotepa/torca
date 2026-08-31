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
      'The Iroh peer connection is currently unavailable.',
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
      kind: 'error:RUNTIME_UNAVAILABLE',
      messageKey: 'runtime.unavailable',
    );
    expect(
      BridgeErrorPresenter.message(result),
      'The Iroh communication runtime is currently unavailable.',
    );
  });

  test('route refresh is presented as a retryable provider-neutral state', () {
    const result = BridgeResultDto(
      ok: false,
      kind: 'error:runtime.route_refresh_required',
    );
    expect(
      BridgeErrorPresenter.message(result),
      'The communication route is changing. Please retry when it is refreshed.',
    );
  });

  test('pairing approval failures retain actionable diagnostics', () {
    const result = BridgeResultDto(
      ok: false,
      kind: 'error:pairing.approval_invalid',
    );
    expect(
      BridgeErrorPresenter.message(result),
      'The pairing approval could not be authenticated.',
    );
  });

  test(
    'expired pairing state is distinct from a generic operation failure',
    () {
      const result = BridgeResultDto(
        ok: false,
        kind: 'error:pairing.session_not_found',
      );
      expect(
        BridgeErrorPresenter.message(result),
        'This pairing session is no longer available.',
      );
    },
  );
}
