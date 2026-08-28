import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/gateway/engine_gateway.dart';
import 'package:torca_app/generated/torca_contract.dart';

Map<String, dynamic> _metadata({
  String provider = 'iroh',
  String? providerProfile = 'direct',
  Object? providerEndpointHash,
}) => <String, dynamic>{
  'metadataSchema': 2,
  'productVersion': '0.2.0-alpha.0',
  'buildId': 'build',
  'sourceCommit': 'commit',
  'sourceFingerprint': 'fingerprint',
  'communicationProvider': provider,
  if (providerProfile != null) 'providerProfile': providerProfile,
  'providerEndpointHash': providerEndpointHash,
  'targetPlatform': 'android',
  'targetArchitecture': 'aarch64',
  'contractSchema': 23,
  'wireVersion': 1,
};

void main() {
  test(
    'provider route state is decoded independently from communication state',
    () {
      final status = TransportStatusDto.fromJson(const <String, dynamic>{
        'communication': <String, dynamic>{'state': 'ready'},
        'providerRouteState': 'stale',
      });
      expect(status.typedProviderRouteState, ProviderRouteState.stale);
    },
  );
  test('direct provider accepts a null endpoint hash', () {
    final info = ClientBuildInfo.fromJson(_metadata());
    expect(info.communicationProvider, 'iroh');
    expect(info.providerEndpointHash, isNull);
    expect(info.providerEndpointRequired, isFalse);
    expect(info.providerProfile, 'direct');
  });

  test('metadata rejects an unknown communication provider', () {
    expect(
      () => ClientBuildInfo.fromJson(_metadata(provider: 'memory')),
      throwsA(isA<FormatException>()),
    );
  });
}
