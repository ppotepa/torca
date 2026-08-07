import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  test('read intent can suppress the remote receipt', () {
    const command = MarkConversationReadCommandDto(
      conversationIdHex: '00000000000000000000000000000001',
      sendReceipt: false,
    );
    expect(command.sendReceipt, isFalse);
  });

  test('read receipt remains enabled by default for compatibility', () {
    const command = MarkConversationReadCommandDto(
      conversationIdHex: '00000000000000000000000000000001',
    );
    expect(command.sendReceipt, isTrue);
  });
}
