import 'package:flutter/foundation.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

class ConversationTimelineController extends ChangeNotifier {
  ConversationTimelineController({
    required EngineGateway gateway,
    required String conversationId,
    this.pageSize = 100,
  }) : _gateway = gateway,
       _conversationId = conversationId;

  EngineGateway _gateway;
  String _conversationId;
  final int pageSize;

  List<MessageDto> _messages = const <MessageDto>[];
  bool _hasMore = true;
  bool _loading = false;
  int _generation = 0;

  List<MessageDto> get messages => _messages;
  bool get hasMore => _hasMore;
  bool get loading => _loading;
  String get conversationId => _conversationId;

  Future<void> initialize() => _loadLatest(replace: true);

  Future<void> reset({
    required EngineGateway gateway,
    required String conversationId,
  }) async {
    _generation++;
    _gateway = gateway;
    _conversationId = conversationId;
    _messages = const <MessageDto>[];
    _hasMore = true;
    _loading = false;
    notifyListeners();
    await _loadLatest(replace: true);
  }

  Future<void> refreshLatest() => _loadLatest(replace: false);

  Future<int> loadOlder() async {
    if (_loading || !_hasMore || _messages.isEmpty) return 0;
    final generation = _generation;
    _loading = true;
    notifyListeners();
    try {
      final page = await conversationPageFor(
        _gateway,
        _conversationId,
        before: _messages.first,
        limit: pageSize,
      );
      if (generation != _generation) return 0;
      final previousCount = _messages.length;
      _messages = _merge(page.messages, _messages);
      _hasMore = page.hasMore;
      return _messages.length - previousCount;
    } finally {
      if (generation == _generation) {
        _loading = false;
        notifyListeners();
      }
    }
  }

  Future<ConversationPageDto> search(String query, {int limit = 100}) =>
      searchConversationFor(_gateway, _conversationId, query, limit: limit);

  Future<void> _loadLatest({required bool replace}) async {
    if (_loading) return;
    final generation = _generation;
    _loading = true;
    notifyListeners();
    try {
      final page = await conversationPageFor(
        _gateway,
        _conversationId,
        limit: pageSize,
      );
      if (generation != _generation) return;
      if (replace || _messages.isEmpty || page.messages.isEmpty) {
        _messages = page.messages;
      } else {
        _messages = _merge(_messages, page.messages);
      }
      // A latest page shorter than the configured size proves the entire conversation is loaded.
      // Once older pages have already been loaded, retain their hasMore state unless the history
      // was cleared and the latest page is now empty.
      if (replace || _messages.length <= pageSize || page.messages.isEmpty) {
        _hasMore = page.hasMore;
      }
    } finally {
      if (generation == _generation) {
        _loading = false;
        notifyListeners();
      }
    }
  }

  List<MessageDto> _merge(List<MessageDto> first, List<MessageDto> second) {
    final byId = <String, MessageDto>{};
    for (final message in first) {
      byId[message.id] = message;
    }
    for (final message in second) {
      byId[message.id] = message;
    }
    final result = byId.values.toList(growable: false)
      ..sort((a, b) {
        final byTime = a.createdAtMs.compareTo(b.createdAtMs);
        return byTime != 0 ? byTime : a.id.compareTo(b.id);
      });
    return result;
  }
}
