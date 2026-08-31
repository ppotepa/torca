import 'package:avatar_genome/avatar_genome.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_avatar/torca_avatar.dart';

void main() {
  testWidgets('contact placeholder genome renders without remote identity', (
    tester,
  ) async {
    final generated = AvatarGenerator().generate(
      AvatarRequest(seed: 'contact-placeholder-seed'),
    );
    final envelope = AvatarGenomeCodec.encode(generated.genome);
    await tester.pumpWidget(
      MaterialApp(
        home: TorcaDeviceAvatar(
          identityId: '',
          fallbackIdentityId: 'contact-record-42',
          label: 'Alice',
          envelope: envelope,
        ),
      ),
    );

    expect(
      find.byKey(const ValueKey<String>('torca-avatar-loader')),
      findsOneWidget,
    );
  });

  testWidgets(
    'stable device avatar renders before the first identity snapshot',
    (tester) async {
      final generated = AvatarGenerator().generate(
        AvatarRequest(seed: 'profile-before-identity'),
      );
      final envelope = AvatarGenomeCodec.encode(generated.genome);
      await tester.runAsync(
        () => AvatarRepository.instance.imageBytes(
          identityId: 'local-device',
          size: 96,
          envelope: envelope,
        ),
      );
      await tester.pumpWidget(
        MaterialApp(
          home: TorcaDeviceAvatar(
            identityId: null,
            label: 'New profile',
            stableDevice: true,
            size: 160,
            envelope: envelope,
            presentation: AvatarActivityPresentation(
              AvatarAnimationState.sleepy,
            ),
          ),
        ),
      );

      // The stable-device path must start before the native identity snapshot
      // exists. Sprite generation itself is covered by generator smoke tests
      // and may complete outside the widget test's fake clock.
      expect(
        find.byKey(const ValueKey<String>('torca-avatar-loader')),
        findsOneWidget,
      );
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(seconds: 2)),
      );
      await tester.pumpAndSettle();
      expect(
        find
                .byKey(const ValueKey<String>('torca-avatar-preview'))
                .evaluate()
                .length +
            find
                .byKey(const ValueKey<String>('torca-avatar-sprite'))
                .evaluate()
                .length,
        1,
      );

      await tester.pumpWidget(const SizedBox.shrink());
      expect(AvatarFrameClock.instance.clients, 0);
    },
  );

  testWidgets('sleeping and hidden avatars consume no shared clock client', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: TorcaDeviceAvatar(
          identityId: 'sleeping-peer',
          label: 'Sleeping peer',
          presentation: AvatarActivityPresentation(AvatarAnimationState.sleepy),
        ),
      ),
    );
    await tester.pump(const Duration(seconds: 2));
    expect(AvatarFrameClock.instance.clients, 0);

    await tester.pumpWidget(
      const MaterialApp(
        home: TickerMode(
          enabled: false,
          child: TorcaDeviceAvatar(
            identityId: 'hidden-peer',
            label: 'Hidden peer',
          ),
        ),
      ),
    );
    await tester.pump(const Duration(seconds: 2));
    expect(AvatarFrameClock.instance.clients, 0);
  });
}
