import 'dart:async';

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
  bool _refreshing = false;
  bool _refreshPending = false;
  bool _disposed = false;
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
    if (_disposed) return;
    _generation++;
    _gateway = gateway;
    _conversationId = conversationId;
    _messages = const <MessageDto>[];
    _hasMore = true;
    _loading = false;
    _notifyChanged();
    await _loadLatest(replace: true);
  }

  Future<void> refreshLatest() => _loadLatest(replace: false);

  Future<int> loadOlder() async {
    if (_disposed ||
        _loading ||
        _refreshing ||
        !_hasMore ||
        _messages.isEmpty) {
      return 0;
    }
    final generation = _generation;
    _loading = true;
    _notifyChanged();
    try {
      final page = await conversationPageFor(
        _gateway,
        _conversationId,
        before: _messages.first,
        limit: pageSize,
      );
      if (_disposed || generation != _generation) return 0;
      final previousCount = _messages.length;
      _messages = _merge(page.messages, _messages);
      _hasMore = page.hasMore;
      return _messages.length - previousCount;
    } on TimeoutException catch (error, stackTrace) {
      debugPrint('older conversation history temporarily unavailable: $error');
      debugPrintStack(stackTrace: stackTrace);
      return 0;
    } finally {
      if (generation == _generation) {
        _loading = false;
        _notifyChanged();
      }
    }
  }

  Future<ConversationPageDto> search(String query, {int limit = 100}) =>
      searchConversationFor(_gateway, _conversationId, query, limit: limit);

  Future<void> _loadLatest({required bool replace}) async {
    if (_disposed) return;
    if (_loading || _refreshing) {
      _refreshPending = true;
      return;
    }
    final generation = _generation;
    var replaceCurrent = replace;
    _refreshing = true;
    try {
      do {
        _refreshPending = false;
        // A background refresh must not replace a stable empty/data state with
        // a spinner. The previous implementation toggled this flag on every
        // global snapshot (roughly every 250ms), which made "No messages yet"
        // visibly flicker in an otherwise idle conversation.
        final initialLoad = replaceCurrent && _messages.isEmpty;
        if (initialLoad) {
          _loading = true;
          _notifyChanged();
        }
        try {
          final page = await conversationPageFor(
            _gateway,
            _conversationId,
            limit: pageSize,
          );
          if (_disposed || generation != _generation) return;
          if (replaceCurrent || _messages.isEmpty || page.messages.isEmpty) {
            _messages = page.messages;
          } else {
            _messages = _merge(_messages, page.messages);
          }
          if (replaceCurrent ||
              _messages.length <= pageSize ||
              page.messages.isEmpty) {
            _hasMore = page.hasMore;
          }
          _notifyChanged();
        } on TimeoutException catch (error, stackTrace) {
          // Attachment transfers can temporarily occupy the native
          // communication actor. Keep the last stable timeline visible and
          // retry on the next snapshot instead of surfacing an unhandled
          // exception from the Flutter worker.
          debugPrint('conversation history temporarily unavailable: $error');
          debugPrintStack(stackTrace: stackTrace);
        } finally {
          if (generation == _generation && initialLoad) {
            _loading = false;
            _notifyChanged();
          }
        }
        replaceCurrent = false;
      } while (!_disposed && generation == _generation && _refreshPending);
    } finally {
      _refreshing = false;
    }
  }

  void _notifyChanged() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _generation++;
    _refreshPending = false;
    super.dispose();
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
