import 'package:avatar_genome/avatar_genome.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:torca_avatar/torca_avatar.dart';

void main() {
  test('generator accepts the Torca device request', () {
    final result = AvatarGenerator().generate(
      AvatarRequest(seed: 'torca-device-v1:device-identity-a'),
    );
    expect(result.validation.isValid, isTrue);
  });

  test('repository renders a cached image', () async {
    final bytes = await AvatarRepository.instance.imageBytes(
      identityId: 'device-identity-a',
      size: 48,
    );
    expect(bytes, isNotNull);
    expect(bytes, isNotEmpty);
  });

  test('repository renders an exchanged genome at profile size', () async {
    final generated = AvatarGenerator().generate(
      AvatarRequest(seed: 'profile-envelope'),
    );
    final envelope = AvatarGenomeCodec.encode(generated.genome);
    final bytes = await AvatarRepository.instance.imageBytes(
      identityId: 'profile-envelope',
      size: 96,
      envelope: envelope,
    );
    expect(bytes, isNotNull);
    expect(bytes, isNotEmpty);
  });

  test('physical device seed is stable and pseudonymous', () {
    final first = AvatarDeviceSeed.fromPlatformIdentifier('DEVICE-42');
    final afterReinstall = AvatarDeviceSeed.fromPlatformIdentifier('DEVICE-42');
    expect(afterReinstall, first);
    expect(first, isNot(contains('DEVICE-42')));
    expect(first, hasLength(64));
  });

  test('avatar presentation uses explicit activity priority', () {
    expect(
      AvatarActivityPresentation.resolve(
        talking: true,
        attention: true,
        online: false,
      ).state,
      AvatarAnimationState.talk,
    );
    expect(
      AvatarActivityPresentation.resolve(blocked: true, talking: true).state,
      AvatarAnimationState.sad,
    );
    expect(
      AvatarActivityPresentation.resolve(online: false).state,
      AvatarAnimationState.sleepy,
    );
    expect(
      AvatarActivityPresentation.fromSignals(
        const AvatarPresentationSignals(
          presence: AvatarPresence.offline,
          condition: AvatarCondition.reconnecting,
        ),
      ).state,
      AvatarAnimationState.happy,
    );
    expect(
      const AvatarActivityPresentation(AvatarAnimationState.sleepy).animates,
      isFalse,
    );
  });

  test('every Torca animation maps to one supported distinct face clip', () {
    const supported = <String>{
      'laugh',
      'talk',
      'smirk',
      'angry',
      'sleepy',
      'curious',
      'proud',
      'sad',
      'surprised',
      'evil',
      'happy',
      'bashful',
      'confused',
    };
    final clips = AvatarAnimationState.values
        .map((state) => state.generatorAnimation)
        .toList();
    expect(clips, everyElement(isIn(supported)));
    expect(clips.toSet(), hasLength(AvatarAnimationState.values.length));
    expect(
      AvatarAnimationState.talk.generatorOverrides['v4.mouthMotionStyle'],
      'talkNormal',
    );
  });

  test('repository precompiles and caches animated sprite sheets', () async {
    final envelope = await AvatarRepository.instance.envelopeForIdentity(
      'animated-device',
    );
    final first = await AvatarRepository.instance.spriteSheet(
      identityId: 'animated-device',
      size: 48,
      animation: AvatarAnimationState.talk,
      envelope: envelope,
    );
    final second = await AvatarRepository.instance.spriteSheet(
      identityId: 'animated-device',
      size: 48,
      animation: AvatarAnimationState.talk,
      envelope: envelope,
    );
    expect(first, isNotNull);
    expect(first!.bytes, isNotEmpty);
    expect(first.frameCount, AvatarAnimationState.talk.frameCount);
    expect(identical(first, second), isTrue);
  });

  test('genome envelope round trips and is content addressed', () {
    final result = AvatarGenerator(
      cacheCapacity: 1,
    ).generate(AvatarRequest(seed: 'codec-test-device'));
    final envelope = AvatarGenomeCodec.encode(result.genome);
    final decoded = AvatarGenomeCodec.decode(envelope);
    expect(decoded.seed, result.genome.seed);
    expect(envelope.genomeHash, isNotEmpty);
    expect(envelope.compressedGenome.length, lessThan(32 * 1024));
  });

  test(
    'stable device seed keeps a pinned genome across application releases',
    () {
      final stableSeed = AvatarDeviceSeed.fromPlatformIdentifier('DEVICE-42');
      final result = AvatarGenerator(cacheCapacity: 1).generate(
        AvatarRequest(
          seed: 'torca-device-v1:device-$stableSeed',
          overrides: const <String, Object>{'colors.colorBudget': '32'},
        ),
      );
      final envelope = AvatarGenomeCodec.encode(result.genome);
      expect(
        envelope.genomeHash,
        '2bacb9d506a119d65c523be8e89b0e453426c740c1a68b2ce3f56e1b2d8db52a',
      );
    },
  );

  test('tampered genome payload is rejected before rendering', () {
    final result = AvatarGenerator(
      cacheCapacity: 1,
    ).generate(AvatarRequest(seed: 'tamper-test-device'));
    final envelope = AvatarGenomeCodec.encode(result.genome);
    envelope.compressedGenome[0] ^= 0x01;
    expect(
      () => AvatarGenomeCodec.decode(envelope),
      throwsA(isA<FormatException>()),
    );
  });
}
