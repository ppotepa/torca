import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/generated/torca_contract.dart';

void main() {
  test('runtime poll carries the notification cursor in the canonical query', () {
    final encoded = RuntimeRequestDto.runtimePoll(42).encode('poll-1');
    final value = jsonDecode(encoded) as Map<String, dynamic>;
    expect(value['kind'], 'query');
    expect(value['name'], 'runtime.poll');
    expect((value['payload'] as Map<String, dynamic>)['afterCursor'], 42);
  });
}
