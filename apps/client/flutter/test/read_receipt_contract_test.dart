import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  test('read intent is application-owned', () {
    const command = MarkConversationReadCommandDto(
      conversationIdHex: '00000000000000000000000000000001',
    );
    expect(command.conversationIdHex, isNotEmpty);
  });
}
