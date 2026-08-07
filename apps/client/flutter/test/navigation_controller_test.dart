import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/navigation/app_navigation_controller.dart';

void main() {
  test('new pairing navigation requests are monotonic', () {
    final navigation = AppNavigationController();
    expect(navigation.newPairingRequest.value, 0);
    navigation.requestNewPairing();
    navigation.requestNewPairing();
    expect(navigation.newPairingRequest.value, 2);
  });
}
