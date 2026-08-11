import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/widgets/operation_tracker.dart';

void main() {
  test('operation tracker rejects duplicate keys until completion', () async {
    final tracker = OperationTracker();
    final completer = Completer<void>();
    var calls = 0;

    final first = tracker.run('message:send', () async {
      calls++;
      await completer.future;
    });
    final second = await tracker.run('message:send', () async {
      calls++;
    });

    expect(second, isFalse);
    expect(tracker.isActive('message:send'), isTrue);
    expect(calls, 1);

    completer.complete();
    expect(await first, isTrue);
    expect(tracker.isActive('message:send'), isFalse);
    tracker.dispose();
  });

  test('completion after dispose does not notify a closed tracker', () async {
    final tracker = OperationTracker();
    final completer = Completer<void>();
    final operation = tracker.run('message:send', () => completer.future);

    tracker.dispose();
    completer.complete();

    expect(await operation, isTrue);
    expect(await tracker.run('message:retry', () async {}), isFalse);
  });
}
