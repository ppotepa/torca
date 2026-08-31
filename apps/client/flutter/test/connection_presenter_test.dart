import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/widgets/connection_state_presenter.dart';
import 'package:torca_ui/torca_ui.dart';

void main() {
  const icons = TorcaIconSet.modern;
  test('peer states have one canonical presentation mapping', () {
    expect(
      ConnectionStatePresenter.peer(
        state: 'ready',
        blocked: false,
        icons: icons,
      ).label,
      'Direct Iroh contact',
    );
    expect(
      ConnectionStatePresenter.peer(
        state: 'handshaking',
        blocked: false,
        icons: icons,
      ).label,
      'Connecting',
    );
    expect(
      ConnectionStatePresenter.peer(
        state: 'reconnecting',
        blocked: false,
        icons: icons,
      ).label,
      'Reconnecting',
    );
    expect(
      ConnectionStatePresenter.peer(
        state: 'disconnected',
        blocked: false,
        icons: icons,
      ).label,
      'Peer is offline',
    );
    expect(
      ConnectionStatePresenter.peer(
        state: 'ready',
        blocked: true,
        icons: icons,
      ).label,
      'Blocked',
    );
  });

  test('Iroh states have one canonical presentation mapping', () {
    expect(ConnectionStatePresenter.iroh('ready', icons).label, 'Iroh ready');
    expect(
      ConnectionStatePresenter.iroh('starting', icons).label,
      'Iroh starting',
    );
    expect(
      ConnectionStatePresenter.iroh('reconnecting', icons).label,
      'Iroh reconnecting',
    );
  });

  test('peer presentation uses the selected provider', () {
    expect(
      ConnectionStatePresenter.peer(
        state: 'ready',
        blocked: false,
        provider: 'iroh',
        icons: icons,
      ).label,
      'Direct Iroh contact',
    );
  });
}
