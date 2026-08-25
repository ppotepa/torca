import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';

Map<String, dynamic> _metadata({
  String provider = 'iroh',
  Object? providerEndpointHash,
  Object? legacyEndpointHash,
}) => <String, dynamic>{
  'metadataSchema': 2,
  'productVersion': '0.2.0-alpha.0',
  'buildId': 'build',
  'sourceCommit': 'commit',
  'sourceFingerprint': 'fingerprint',
  'communicationProvider': provider,
  'providerEndpointHash': providerEndpointHash,
  if (legacyEndpointHash != null) 'relayEndpointHash': legacyEndpointHash,
  'targetPlatform': 'android',
  'targetArchitecture': 'aarch64',
  'contractSchema': 23,
  'wireVersion': 1,
};

void main() {
  test('direct provider accepts a null endpoint hash', () {
    final info = ClientBuildInfo.fromJson(_metadata());
    expect(info.communicationProvider, 'iroh');
    expect(info.providerEndpointHash, isNull);
  });

  test('managed provider reads the provider endpoint hash', () {
    final info = ClientBuildInfo.fromJson(
      _metadata(provider: 'tor', providerEndpointHash: 'sha256'),
    );
    expect(info.communicationProvider, 'tor');
    expect(info.providerEndpointHash, 'sha256');
    expect(info.relayEndpointHash, 'sha256');
  });

  test('legacy native metadata remains readable during migration', () {
    final value = _metadata(
      providerEndpointHash: null,
      legacyEndpointHash: 'legacy',
    );
    value.remove('providerEndpointHash');
    value.remove('metadataSchema');
    final info = ClientBuildInfo.fromJson(value);
    expect(info.communicationProvider, 'iroh');
    expect(info.providerEndpointHash, 'legacy');
  });

  test('metadata rejects an unknown communication provider', () {
    expect(
      () => ClientBuildInfo.fromJson(_metadata(provider: 'onion-v2')),
      throwsA(isA<FormatException>()),
    );
  });
}
