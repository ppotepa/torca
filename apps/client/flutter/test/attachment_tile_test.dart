import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';
import 'package:torca_app/theme/app_theme.dart';
import 'package:torca_app/widgets/attachment_tile.dart';

void main() {
  test('byte formatter is human readable', () {
    expect(formatBytes(0), '0 B');
    expect(formatBytes(1024), '1.00 KiB');
    expect(formatBytes(4 * 1024 * 1024), '4.00 MiB');
  });

  testWidgets('attachment tile shows transfer progress and actions', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: AttachmentTile(
            attachment: const AttachmentDto(
              id: '01',
              messageId: '02',
              name: 'photo.jpg',
              mediaType: 'image/jpeg',
              size: 4 * 1024 * 1024,
              status: 'sending',
              offset: 1024 * 1024,
            ),
            onRetry: () {},
            onCancel: () {},
            onOpen: () {},
            onSave: () {},
          ),
        ),
      ),
    );

    expect(find.text('photo.jpg'), findsOneWidget);
    expect(find.textContaining('4.00 MiB'), findsWidgets);
    expect(find.text('1.00 MiB / 4.00 MiB'), findsOneWidget);
    expect(find.text('Cancel'), findsOneWidget);
    expect(find.byIcon(Icons.image_outlined), findsOneWidget);
  });
}
