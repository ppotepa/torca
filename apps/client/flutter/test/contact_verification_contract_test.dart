import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  test('contact verification defaults to unverified', () {
    const contact = ContactDto(
      id: '1',
      onionAddress: 'example.onion',
      status: 'active',
      connectionState: 'ready',
    );
    expect(contact.verificationStatus, 'unverified');
    expect(contact.verifiedAtMs, isNull);
  });

  test('contact verification can carry a local verification timestamp', () {
    const contact = ContactDto(
      id: '1',
      onionAddress: 'example.onion',
      status: 'active',
      connectionState: 'ready',
      verificationStatus: 'verified',
      verifiedAtMs: 123,
    );
    expect(contact.verificationStatus, 'verified');
    expect(contact.verifiedAtMs, 123);
  });
}
