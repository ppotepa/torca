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
      'Direct P2P over Tor',
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

  test('Tor states have one canonical presentation mapping', () {
    expect(ConnectionStatePresenter.tor('ready', icons).label, 'Tor ready');
    expect(
      ConnectionStatePresenter.tor('starting', icons).label,
      'Tor starting',
    );
    expect(
      ConnectionStatePresenter.tor('reconnecting', icons).label,
      'Tor reconnecting',
    );
  });
}
