import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:torca_app/widgets/operation_tracker.dart';

void main() {
  test('operation tracker does not notify after dispose', () async {
    final tracker = OperationTracker();
    var notifications = 0;
    tracker.addListener(() => notifications++);
    final gate = Completer<void>();
    final operation = tracker.run('send', () => gate.future);
    expect(notifications, 1);
    tracker.dispose();
    gate.complete();
    expect(await operation, isTrue);
    expect(notifications, 1);
  });
}
