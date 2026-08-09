import 'package:flutter/foundation.dart';

/// Small presentation-side gate for asynchronous user actions.
///
/// It prevents duplicate invocation while keeping operation ownership in the
/// backend. Keys are scoped by the caller (for example `message:<id>:retry`).
class OperationTracker extends ChangeNotifier {
  final Set<String> _active = <String>{};

  bool isActive(String key) => _active.contains(key);

  bool anyWithPrefix(String prefix) =>
      _active.any((key) => key.startsWith(prefix));

  Future<bool> run(String key, Future<void> Function() action) async {
    if (!_active.add(key)) return false;
    notifyListeners();
    try {
      await action();
      return true;
    } finally {
      _active.remove(key);
      notifyListeners();
    }
  }
}
