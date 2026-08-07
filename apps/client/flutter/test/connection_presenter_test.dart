import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/widgets/connection_state_presenter.dart';

void main() {
  test('peer states have one canonical presentation mapping', () {
    expect(
      ConnectionStatePresenter.peer(state: 'ready', blocked: false).label,
      'Direct P2P over Tor',
    );
    expect(
      ConnectionStatePresenter.peer(state: 'handshaking', blocked: false).label,
      'Connecting',
    );
    expect(
      ConnectionStatePresenter.peer(state: 'reconnecting', blocked: false).label,
      'Reconnecting',
    );
    expect(
      ConnectionStatePresenter.peer(state: 'disconnected', blocked: false).label,
      'Offline',
    );
    expect(
      ConnectionStatePresenter.peer(state: 'ready', blocked: true).label,
      'Blocked',
    );
  });

  test('Tor states have one canonical presentation mapping', () {
    expect(ConnectionStatePresenter.tor('ready').label, 'Tor ready');
    expect(ConnectionStatePresenter.tor('starting').label, 'Tor starting');
    expect(ConnectionStatePresenter.tor('reconnecting').label, 'Tor reconnecting');
  });
}
