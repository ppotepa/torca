import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:open_filex/open_filex.dart';
import 'package:torca_attachment_processing/torca_attachment_processing.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../platform/platform_capabilities.dart';
import '../widgets/attachment_tile.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/conversation_header.dart';
import '../widgets/message_actions.dart';
import '../widgets/message_bubble.dart';
import '../widgets/operation_tracker.dart';
import '../widgets/runtime_network_status.dart';
import 'connection_details_screen.dart';
import 'conversation_timeline_controller.dart';

class ConversationScreen extends StatelessWidget {
  const ConversationScreen({
    required this.gateway,
    required this.conversation,
    super.key,
  });
  final EngineGateway gateway;
  final ConversationDto conversation;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: gateway.snapshots,
    builder: (context, snapshot, _) {
      final contact = _contactFor(snapshot, conversation);
      return Scaffold(
        appBar: RuntimeAppBar(
          titleSpacing: 0,
          title: ConversationHeader(
            contact: contact,
            compact: true,
            onConnectionDetails: contact == null
                ? () {}
                : () => _openConnectionDetails(context, contact.id),
          ),
        ),
        body: ConversationPane(
          gateway: gateway,
          conversation: conversation,
          showHeader: false,
        ),
      );
    },
  );

  void _openConnectionDetails(BuildContext context, String contactId) {
    Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (_) =>
            ConnectionDetailsScreen(gateway: gateway, contactId: contactId),
      ),
    );
  }
}

class ConversationPane extends StatefulWidget {
  const ConversationPane({
    required this.gateway,
    required this.conversation,
    this.showHeader = true,
    super.key,
  });
  final EngineGateway gateway;
  final ConversationDto conversation;
  final bool showHeader;

  @override
  State<ConversationPane> createState() => _ConversationPaneState();
}

class _ConversationPaneState extends State<ConversationPane>
    with WidgetsBindingObserver {
  final TextEditingController _controller = TextEditingController();
  final TextEditingController _searchController = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  final OperationTracker _operations = OperationTracker();
  final Map<String, String> _drafts = <String, String>{};
  final AttachmentProcessor _attachmentProcessor = const AttachmentProcessor();
  final List<_PendingAttachment> _pendingAttachments = <_PendingAttachment>[];

  late ConversationTimelineController _timeline;
  Timer? _searchDebounce;
  List<MessageDto> _searchResults = const <MessageDto>[];
  bool _searching = false;
  bool _searchBusy = false;
  bool _markingRead = false;
  bool _loadingOlder = false;
  bool _showJumpToLatest = false;
  int _jumpMessageCount = 0;
  int _lastActivityAtMs = 0;
  String? _unreadBoundaryMessageId;
  MessageDto? _replyingTo;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _timeline = _newTimeline();
    _timeline.addListener(_timelineChanged);
    _operations.addListener(_operationChanged);
    _scrollController.addListener(_scrollChanged);
    widget.gateway.snapshots.addListener(_snapshotChanged);
    _lastActivityAtMs = _conversationSummary()?.lastActivityAtMs ?? 0;
    unawaited(_initializeTimeline());
  }

  ConversationTimelineController _newTimeline() =>
      ConversationTimelineController(
        gateway: widget.gateway,
        conversationId: widget.conversation.id,
      );

  Future<void> _initializeTimeline() async {
    await _timeline.initialize();
    // The runtime may persist an inbound message while the first page is
    // loading. Refresh once more after the initial read so a snapshot event
    // that raced with initialization cannot leave the pane stale.
    await _timeline.refreshLatest();
    if (!mounted) return;
    _captureUnreadBoundary();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _scrollToBottom(jump: true);
      unawaited(_markReadIfNeeded());
    });
  }

  @override
  void didUpdateWidget(covariant ConversationPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.gateway != widget.gateway) {
      oldWidget.gateway.snapshots.removeListener(_snapshotChanged);
      widget.gateway.snapshots.addListener(_snapshotChanged);
    }
    if (oldWidget.gateway != widget.gateway ||
        oldWidget.conversation.id != widget.conversation.id) {
      _drafts[oldWidget.conversation.id] = _controller.text;
      _controller.text = _drafts[widget.conversation.id] ?? '';
      _replyingTo = null;
      _searching = false;
      _searchController.clear();
      _searchResults = const <MessageDto>[];
      _showJumpToLatest = false;
      _timeline.removeListener(_timelineChanged);
      _timeline.dispose();
      _timeline = _newTimeline();
      _timeline.addListener(_timelineChanged);
      _lastActivityAtMs = _conversationSummary()?.lastActivityAtMs ?? 0;
      _unreadBoundaryMessageId = null;
      unawaited(_initializeTimeline());
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(_markReadIfNeeded());
    });
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    widget.gateway.snapshots.removeListener(_snapshotChanged);
    _timeline.removeListener(_timelineChanged);
    _timeline.dispose();
    for (final pending in _pendingAttachments) {
      unawaited(pending.prepared.dispose());
    }
    _pendingAttachments.clear();
    _searchDebounce?.cancel();
    _searchController.dispose();
    _scrollController.removeListener(_scrollChanged);
    _operations.removeListener(_operationChanged);
    _operations.dispose();
    _scrollController.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _timelineChanged() {
    if (!mounted) return;
    setState(() {});
  }

  void _operationChanged() {
    if (mounted) setState(() {});
  }

  void _captureUnreadBoundary() {
    _unreadBoundaryMessageId = _timeline.messages
        .where(
          (message) =>
              message.typedDirection == MessageDirection.inbound &&
              message.typedStatus == MessageStatus.delivered,
        )
        .map((message) => message.id)
        .firstOrNull;
  }

  void _scrollChanged() {
    if (_scrollController.hasClients &&
        _scrollController.position.pixels <= 180 &&
        !_loadingOlder &&
        _timeline.hasMore &&
        !_searching) {
      unawaited(_loadOlder());
    }
    if (!_nearBottom()) return;
    unawaited(_markReadIfNeeded());
    if (_showJumpToLatest) {
      setState(() {
        _showJumpToLatest = false;
        _jumpMessageCount = 0;
      });
    }
  }

  Future<void> _loadOlder() async {
    if (_loadingOlder || !_timeline.hasMore || !_scrollController.hasClients)
      return;
    _loadingOlder = true;
    final beforePixels = _scrollController.position.pixels;
    final beforeExtent = _scrollController.position.maxScrollExtent;
    try {
      final loaded = await _timeline.loadOlder();
      if (!mounted || loaded == 0) return;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || !_scrollController.hasClients) return;
        final delta = _scrollController.position.maxScrollExtent - beforeExtent;
        _scrollController.jumpTo(
          (beforePixels + delta).clamp(
            0.0,
            _scrollController.position.maxScrollExtent,
          ),
        );
      });
    } finally {
      if (mounted) setState(() => _loadingOlder = false);
    }
  }

  void _snapshotChanged() {
    final summary = _conversationSummary();
    final activity = summary?.lastActivityAtMs ?? 0;
    final activityChanged = activity != _lastActivityAtMs;
    _lastActivityAtMs = activity;
    final follow = _nearBottom();
    final beforeCount = _timeline.messages.length;
    unawaited(() async {
      await _timeline.refreshLatest();
      if (!mounted) return;
      final count = _timeline.messages.length;
      if (activityChanged &&
          count > beforeCount &&
          _unreadBoundaryMessageId == null) {
        _captureUnreadBoundary();
      }
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        if (follow) {
          _scrollToBottom();
          unawaited(_markReadIfNeeded());
        } else if (count > beforeCount) {
          setState(() {
            _showJumpToLatest = true;
            _jumpMessageCount += count - beforeCount;
          });
        }
      });
    }());
  }

  ConversationDto? _conversationSummary() {
    for (final conversation in widget.gateway.snapshots.value.conversations) {
      if (conversation.id == widget.conversation.id) return conversation;
    }
    return null;
  }

  bool _nearBottom() {
    if (!_scrollController.hasClients) return true;
    final position = _scrollController.position;
    return position.maxScrollExtent - position.pixels < 96;
  }

  void _scrollToBottom({bool jump = false}) {
    if (!_scrollController.hasClients) return;
    final target = _scrollController.position.maxScrollExtent;
    if (jump) {
      _scrollController.jumpTo(target);
    } else {
      unawaited(
        _scrollController.animateTo(
          target,
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOut,
        ),
      );
    }
    if (_showJumpToLatest && mounted) {
      setState(() {
        _showJumpToLatest = false;
        _jumpMessageCount = 0;
      });
    }
  }

  Future<void> _markReadIfNeeded() async {
    if (_markingRead || !mounted || _searching) return;
    if (WidgetsBinding.instance.lifecycleState != AppLifecycleState.resumed)
      return;
    if (ModalRoute.of(context)?.isCurrent != true || !_nearBottom()) return;
    final hasDelivered = _timeline.messages.any(
      (message) =>
          message.typedDirection == MessageDirection.inbound &&
          message.typedStatus == MessageStatus.delivered,
    );
    if (!hasDelivered) return;
    _markingRead = true;
    try {
      final result = await widget.gateway.execute(
        MarkConversationReadCommandDto(
          conversationIdHex: widget.conversation.id,
        ),
      );
      if (mounted && !result.ok) {
        _showError(
          BridgeErrorPresenter.message(
            result,
            fallback: 'Could not mark conversation as read',
          ),
        );
      }
    } finally {
      _markingRead = false;
    }
  }

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: widget.gateway.snapshots,
    builder: (context, snapshot, _) {
      final messages = _searching ? _searchResults : _timeline.messages;
      final byId = <String, MessageDto>{
        for (final message in _timeline.messages) message.id: message,
      };
      final reply = _replyingTo;
      final contact = _contactFor(snapshot, widget.conversation);
      final sending = _operations.isActive('message:send');
      final sendingAttachment = _operations.isActive('attachment:send');
      final pickingAttachment = _operations.isActive('attachment:pick');

      return Column(
        children: <Widget>[
          if (widget.showHeader) ...<Widget>[
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 10, 8, 10),
              child: ConversationHeader(
                contact: contact,
                onConnectionDetails: contact == null
                    ? () {}
                    : () => _openConnectionDetails(contact.id),
              ),
            ),
            const Divider(height: 1),
          ],
          _buildSearchBar(),
          Expanded(
            child: Stack(
              children: <Widget>[
                if (_timeline.loading && messages.isEmpty)
                  const Center(child: CircularProgressIndicator())
                else if (messages.isEmpty)
                  Center(
                    child: Padding(
                      padding: const EdgeInsets.all(24),
                      child: Text(
                        _searching
                            ? (_searchController.text.trim().isEmpty
                                  ? 'Type to search this conversation.'
                                  : 'No matching messages.')
                            : 'No messages yet. Messages are sent directly through Tor.',
                        textAlign: TextAlign.center,
                      ),
                    ),
                  )
                else
                  ListView.builder(
                    controller: _scrollController,
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    itemCount: messages.length,
                    itemBuilder: (context, index) {
                      final message = messages[index];
                      final previous = index == 0 ? null : messages[index - 1];
                      final showDate =
                          previous == null || !_sameDay(previous, message);
                      final showUnread =
                          !_searching && message.id == _unreadBoundaryMessageId;
                      final grouped =
                          previous != null &&
                          previous.direction == message.direction &&
                          !showDate &&
                          !showUnread &&
                          (message.createdAtMs - previous.createdAtMs).abs() <
                              5 * 60 * 1000;
                      final quoted = message.replyToMessageId == null
                          ? null
                          : byId[message.replyToMessageId];
                      final attachments = snapshot.attachments
                          .where(
                            (attachment) => attachment.messageId == message.id,
                          )
                          .toList(growable: false);
                      final retryable =
                          message.typedDirection == MessageDirection.outbound &&
                          message.typedStatus == MessageStatus.failed;
                      final attachmentAnnouncement = message.body.startsWith(
                        'Attachment: ',
                      );
                      final retryBusy = _operations.isActive(
                        'message:${message.id}:retry',
                      );

                      return Column(
                        children: <Widget>[
                          if (showDate)
                            _DateSeparator(milliseconds: message.createdAtMs),
                          if (showUnread) const _UnreadSeparator(),
                          MessageBubble(
                            message: message,
                            // Never expose the compatibility announcement
                            // (which contains a path/hash) as chat content.
                            // A typed AttachmentDto is rendered below; until
                            // it arrives, show a safe synchronizing state.
                            showBody: !attachmentAnnouncement,
                            senderLabel:
                                message.typedDirection ==
                                    MessageDirection.outbound
                                ? 'You'
                                : contact?.displayName ?? 'Contact',
                            compactTop: grouped,
                            onLongPress: () => _showMessageActions(message),
                            onSecondaryTapDown: (details) =>
                                _showMessageActions(
                                  message,
                                  globalPosition: details.globalPosition,
                                ),
                            quotedBody: message.replyToMessageId == null
                                ? null
                                : quoted?.body ??
                                      'Original message unavailable',
                            quotedUnavailable:
                                message.replyToMessageId != null &&
                                quoted == null,
                            footer: <Widget>[
                              if (attachmentAnnouncement && attachments.isEmpty)
                                Align(
                                  alignment: Alignment.centerLeft,
                                  child: Text(
                                    'Attachment is syncing…',
                                    style: Theme.of(
                                      context,
                                    ).textTheme.bodySmall,
                                  ),
                                ),
                              if (retryable && !_searching)
                                Align(
                                  alignment: Alignment.centerLeft,
                                  child: TextButton.icon(
                                    onPressed: retryBusy
                                        ? null
                                        : () => _retryMessage(message),
                                    icon: retryBusy
                                        ? const SizedBox(
                                            width: 16,
                                            height: 16,
                                            child: CircularProgressIndicator(
                                              strokeWidth: 2,
                                            ),
                                          )
                                        : Icon(context.torcaIcons.retry),
                                    label: Text(
                                      retryBusy ? 'Retrying…' : 'Retry now',
                                    ),
                                  ),
                                ),
                              ...attachments.map((attachment) {
                                final attachmentBusy = _operations
                                    .anyWithPrefix(
                                      'attachment:${attachment.id}:',
                                    );
                                return AttachmentTile(
                                  attachment: attachment,
                                  operationBusy: attachmentBusy,
                                  onRetry: () => _attachmentCommand(
                                    attachment.id,
                                    'retry',
                                    RetryAttachmentCommandDto(
                                      attachmentIdHex: attachment.id,
                                    ),
                                  ),
                                  onCancel: () => _attachmentCommand(
                                    attachment.id,
                                    'cancel',
                                    CancelAttachmentCommandDto(
                                      attachmentIdHex: attachment.id,
                                    ),
                                  ),
                                  onOpen: () => _openAttachment(attachment),
                                  onSave: () => _saveAttachment(attachment),
                                  loadPreview:
                                      attachment.mediaType.startsWith('image/')
                                      ? () => _loadAttachmentPreview(attachment)
                                      : null,
                                );
                              }),
                            ],
                          ),
                        ],
                      );
                    },
                  ),
                if (_loadingOlder)
                  const Positioned(
                    top: 6,
                    left: 0,
                    right: 0,
                    child: Center(
                      child: SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                    ),
                  ),
                if (_showJumpToLatest && !_searching)
                  Positioned(
                    right: 16,
                    bottom: 12,
                    child: Badge(
                      isLabelVisible: _jumpMessageCount > 0,
                      label: Text('$_jumpMessageCount'),
                      child: FloatingActionButton.small(
                        heroTag: null,
                        tooltip: 'Jump to latest message',
                        onPressed: _scrollToBottom,
                        child: Icon(context.torcaIcons.jumpToLatest),
                      ),
                    ),
                  ),
              ],
            ),
          ),
          const Divider(height: 1),
          SafeArea(
            top: false,
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (_pendingAttachments.isNotEmpty) ...<Widget>[
                    _AttachmentTray(
                      attachments: _pendingAttachments,
                      onRemove: (pending) => setState(() {
                        _pendingAttachments.remove(pending);
                        unawaited(pending.prepared.dispose());
                      }),
                    ),
                    const SizedBox(height: 8),
                  ],
                  if (reply != null) ...<Widget>[
                    _ReplyComposerPreview(
                      message: reply,
                      onCancel: () => setState(() => _replyingTo = null),
                    ),
                    const SizedBox(height: 8),
                  ],
                  Row(
                    children: <Widget>[
                      IconButton(
                        tooltip: 'Attach files',
                        onPressed:
                            pickingAttachment || sendingAttachment || _searching
                            ? null
                            : _pickAttachments,
                        icon: pickingAttachment
                            ? const SizedBox(
                                width: 20,
                                height: 20,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : Icon(context.torcaIcons.attachment),
                      ),
                      const SizedBox(width: 4),
                      Expanded(
                        child: _composerField(
                          sending || _searching,
                          reply != null,
                        ),
                      ),
                      const SizedBox(width: 8),
                      IconButton.filled(
                        tooltip: sending || sendingAttachment
                            ? 'Sending'
                            : 'Send message',
                        onPressed: sending || sendingAttachment || _searching
                            ? null
                            : _sendMessage,
                        icon: sending || sendingAttachment
                            ? const SizedBox(
                                width: 18,
                                height: 18,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : Icon(context.torcaIcons.send),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ],
      );
    },
  );

  Widget _buildSearchBar() {
    if (!_searching) {
      return Align(
        alignment: Alignment.centerRight,
        child: Padding(
          padding: const EdgeInsets.only(right: 8),
          child: IconButton(
            tooltip: 'Search messages',
            onPressed: () => setState(() => _searching = true),
            icon: Icon(context.torcaIcons.search),
          ),
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 8, 8, 8),
      child: Row(
        children: <Widget>[
          Expanded(
            child: TextField(
              controller: _searchController,
              autofocus: true,
              decoration: InputDecoration(
                isDense: true,
                hintText: 'Search this conversation',
                prefixIcon: Icon(context.torcaIcons.search),
              ),
              onChanged: _searchChanged,
            ),
          ),
          if (_searchBusy)
            const Padding(
              padding: EdgeInsets.all(10),
              child: SizedBox(
                width: 18,
                height: 18,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
            )
          else
            IconButton(
              tooltip: 'Close search',
              onPressed: _closeSearch,
              icon: Icon(context.torcaIcons.close),
            ),
        ],
      ),
    );
  }

  void _searchChanged(String value) {
    _searchDebounce?.cancel();
    _searchDebounce = Timer(const Duration(milliseconds: 250), () async {
      final query = value.trim();
      if (!mounted) return;
      if (query.isEmpty) {
        setState(() => _searchResults = const <MessageDto>[]);
        return;
      }
      setState(() => _searchBusy = true);
      try {
        final page = await _timeline.search(query);
        if (!mounted || _searchController.text.trim() != query) return;
        final results = page.messages.toList(growable: false)
          ..sort((a, b) {
            final byTime = a.createdAtMs.compareTo(b.createdAtMs);
            return byTime != 0 ? byTime : a.id.compareTo(b.id);
          });
        setState(() => _searchResults = results);
      } finally {
        if (mounted && _searchController.text.trim() == query) {
          setState(() => _searchBusy = false);
        }
      }
    });
  }

  void _closeSearch() {
    _searchDebounce?.cancel();
    setState(() {
      _searching = false;
      _searchBusy = false;
      _searchResults = const <MessageDto>[];
      _searchController.clear();
    });
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => _scrollToBottom(jump: true),
    );
  }

  Widget _composerField(bool disabled, bool replying) {
    final field = TextField(
      controller: _controller,
      enabled: !disabled,
      minLines: 1,
      maxLines: 5,
      textInputAction: TextInputAction.newline,
      decoration: InputDecoration(labelText: replying ? 'Reply' : 'Message'),
    );
    if (!isTorcaDesktop) return field;
    return CallbackShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.enter): _sendMessage,
        const SingleActivator(LogicalKeyboardKey.enter, shift: true):
            _insertNewline,
      },
      child: field,
    );
  }

  void _insertNewline() {
    final selection = _controller.selection;
    final start = selection.isValid ? selection.start : _controller.text.length;
    final end = selection.isValid ? selection.end : start;
    final text = _controller.text.replaceRange(start, end, '\n');
    _controller.value = TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: start + 1),
    );
  }

  void _openConnectionDetails(String contactId) {
    Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (_) => ConnectionDetailsScreen(
          gateway: widget.gateway,
          contactId: contactId,
        ),
      ),
    );
  }

  Future<void> _retryMessage(MessageDto message) async {
    await _operations.run('message:${message.id}:retry', () async {
      final result = await widget.gateway.execute(
        RetryMessageCommandDto(messageIdHex: message.id),
      );
      if (mounted && !result.ok) {
        _showError(
          BridgeErrorPresenter.message(
            result,
            fallback: 'Could not retry message',
          ),
        );
      }
    });
  }

  Future<void> _showMessageActions(
    MessageDto message, {
    Offset? globalPosition,
  }) async {
    final action = globalPosition == null
        ? await MessageActionMenu.showTouch(context)
        : await MessageActionMenu.showDesktop(context, globalPosition);
    if (!mounted || action == null) return;
    await _applyMessageAction(message, action);
  }

  Future<void> _applyMessageAction(
    MessageDto message,
    MessageAction action,
  ) async {
    switch (action) {
      case MessageAction.reply:
        if (!_searching) setState(() => _replyingTo = message);
      case MessageAction.copy:
        await Clipboard.setData(ClipboardData(text: message.body));
        if (mounted) {
          ScaffoldMessenger.of(
            context,
          ).showSnackBar(const SnackBar(content: Text('Message copied')));
        }
      case MessageAction.details:
        await _showMessageDetails(message);
    }
  }

  Future<void> _showMessageDetails(MessageDto message) => showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('Message details'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          _detail('ID', message.id),
          _detail('Direction', message.direction),
          _detail('Status', _messageStatusLabel(message.status)),
          _detail('Queued / received', _date(message.createdAtMs)),
          if (message.sentAtMs != null)
            _detail('Sent', _date(message.sentAtMs!)),
          if (message.deliveredAtMs != null)
            _detail('Delivered', _date(message.deliveredAtMs!)),
          if (message.readAtMs != null)
            _detail('Read', _date(message.readAtMs!)),
          _detail('Send attempts', '${message.attemptCount}'),
        ],
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Close'),
        ),
      ],
    ),
  );

  Widget _detail(String label, String value) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Text('$label: $value'),
  );

  String _date(int ms) => ms <= 0
      ? 'Unavailable'
      : DateTime.fromMillisecondsSinceEpoch(ms).toLocal().toString();

  Future<void> _sendMessage() async {
    final body = _controller.text.trim();
    if ((body.isEmpty && _pendingAttachments.isEmpty) || _searching) return;
    if (body.isNotEmpty) {
      final replyTo = _replyingTo?.id;
      var sent = false;
      await _operations.run('message:send', () async {
        final result = await widget.gateway.execute(
          QueueMessageCommandDto(
            conversationIdHex: widget.conversation.id,
            body: body,
            replyToMessageId: replyTo,
          ),
        );
        if (!mounted) return;
        if (result.ok) {
          sent = true;
          _controller.clear();
          _drafts.remove(widget.conversation.id);
          setState(() => _replyingTo = null);
          await _timeline.refreshLatest();
          WidgetsBinding.instance.addPostFrameCallback(
            (_) => _scrollToBottom(),
          );
        } else {
          _showError(
            BridgeErrorPresenter.message(
              result,
              fallback: 'Could not queue message',
            ),
          );
        }
      });
      if (!sent) return;
    }
    if (_pendingAttachments.isNotEmpty) await _queuePendingAttachments();
  }

  Future<void> _pickAttachments() async {
    await _operations.run('attachment:pick', () async {
      final picked = await FilePicker.pickFiles(
        allowMultiple: true,
        withData: false,
      );
      if (picked == null || picked.files.isEmpty || !mounted) return;
      final capabilities = capabilitiesFor(widget.gateway);
      final maxBytes = capabilities.maxAttachmentBytes;
      final maximumFiles = capabilities.maxQueuedAttachments;
      final maximumVideoBytes = capabilities.maxVideoAttachmentBytes;
      final remainingSlots = maximumFiles - _pendingAttachments.length;
      if (remainingSlots <= 0) {
        _showError('You can queue at most $maximumFiles attachments.');
        return;
      }
      if (picked.files.length > remainingSlots) {
        _showError('Only $remainingSlots attachment slots remain.');
      }
      final preparedAttachments = <_PendingAttachment>[];
      for (final file in picked.files.take(remainingSlots)) {
        final path = file.path;
        if (path == null || path.isEmpty) {
          _showError('${file.name}: local file path is unavailable');
          continue;
        }
        if (file.size <= 0) {
          _showError('${file.name}: the selected file is empty');
          continue;
        }
        if (file.size > capabilities.maxAttachmentSourceBytes) {
          _showError(
            '${file.name}: maximum source size is '
            '${formatBytes(capabilities.maxAttachmentSourceBytes)}',
          );
          continue;
        }
        PreparedAttachment prepared;
        try {
          prepared = await _attachmentProcessor.prepare(
            sourcePath: path,
            originalName: file.name,
            extension: file.extension,
            maximumBytes: maxBytes,
            maximumVideoBytes: maximumVideoBytes,
          );
        } on AttachmentSizeException catch (error) {
          _showError(
            '${file.name}: maximum size is '
            '${formatBytes(error.maximumBytes)}',
          );
          continue;
        } on AttachmentSelectionException catch (error) {
          _showError('${file.name}: ${error.message}');
          continue;
        } catch (_) {
          _showError('${file.name}: the file could not be processed');
          continue;
        }
        final limit = prepared.kind == AttachmentMediaKind.video
            ? maximumVideoBytes
            : maxBytes;
        if (prepared.size > limit) {
          _showError(
            '${file.name}: maximum ${prepared.kind == AttachmentMediaKind.video ? 'video' : 'attachment'} size is ${formatBytes(limit)}',
          );
          await prepared.dispose();
          continue;
        }
        preparedAttachments.add(_PendingAttachment(file.name, prepared));
      }
      if (preparedAttachments.isEmpty || !mounted) return;
      setState(() => _pendingAttachments.addAll(preparedAttachments));
    });
  }

  Future<void> _queuePendingAttachments() async {
    if (_pendingAttachments.isEmpty) return;
    final pending = List<_PendingAttachment>.of(_pendingAttachments);
    var queued = 0;
    await _operations.run('attachment:send', () async {
      for (final item in pending) {
        final prepared = item.prepared;
        final response = await widget.gateway.execute(
          QueueAttachmentCommandDto(
            conversationIdHex: widget.conversation.id,
            sourcePath: prepared.path,
            name: prepared.name,
            mediaType: prepared.mediaType,
            size: prepared.size,
          ),
        );
        if (!mounted) return;
        if (!response.ok) {
          // Keep the app-owned staging file and tray entry when queueing
          // fails.  Relay/runtime outages are transient; deleting the source
          // here made retry impossible and forced the user to pick the file
          // again.
          _showError(
            '${item.originalName}: ${BridgeErrorPresenter.message(response, fallback: 'Could not queue attachment')}',
          );
          continue;
        }
        await prepared.dispose();
        if (!mounted) return;
        setState(() => _pendingAttachments.remove(item));
        queued++;
      }
      if (queued > 0) await _timeline.refreshLatest();
      if (mounted && queued > 1) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('$queued attachments queued')));
      }
    });
  }

  Future<void> _saveAttachment(AttachmentDto attachment) async {
    await _operations.run('attachment:${attachment.id}:save', () async {
      final path = await FilePicker.saveFile(
        dialogTitle: 'Save attachment',
        fileName: attachment.name,
      );
      if (path == null || !mounted) return;
      final result = await widget.gateway.execute(
        ExportAttachmentCommandDto(
          attachmentIdHex: attachment.id,
          destinationPath: path,
        ),
      );
      if (!mounted) return;
      if (result.ok) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(const SnackBar(content: Text('Attachment saved')));
      } else {
        _showError(
          BridgeErrorPresenter.message(
            result,
            fallback: 'Could not save attachment',
          ),
        );
      }
    });
  }

  Future<String?> _loadAttachmentPreview(AttachmentDto attachment) async {
    final path =
        '${Directory.systemTemp.path}${torcaPathSeparator}torca-preview-${attachment.id}.jpg';
    final file = File(path);
    if (await file.exists() && await file.length() > 0) return path;
    final result = await widget.gateway.execute(
      ExportAttachmentCommandDto(
        attachmentIdHex: attachment.id,
        destinationPath: path,
      ),
    );
    return result.ok && await file.exists() ? path : null;
  }

  Future<void> _openAttachment(AttachmentDto attachment) async {
    await _operations.run('attachment:${attachment.id}:open', () async {
      final ext = _safeExtension(attachment.name);
      final path =
          '${Directory.systemTemp.path}${torcaPathSeparator}torca-${attachment.id}$ext';
      final result = await widget.gateway.execute(
        ExportAttachmentCommandDto(
          attachmentIdHex: attachment.id,
          destinationPath: path,
        ),
      );
      if (!mounted) return;
      if (!result.ok) {
        _showError(
          BridgeErrorPresenter.message(
            result,
            fallback: 'Could not open attachment',
          ),
        );
        return;
      }
      final opened = await OpenFilex.open(path);
      if (mounted && opened.type != ResultType.done) _showError(opened.message);
    });
  }

  String _safeExtension(String name) {
    final dot = name.lastIndexOf('.');
    if (dot < 0 || dot == name.length - 1) return '';
    final value = name.substring(dot);
    return RegExp(r'^\.[A-Za-z0-9]{1,10}$').hasMatch(value)
        ? value.toLowerCase()
        : '';
  }

  Future<void> _attachmentCommand(
    String attachmentId,
    String action,
    BridgeCommandDto command,
  ) async {
    await _operations.run('attachment:$attachmentId:$action', () async {
      final result = await widget.gateway.execute(command);
      if (mounted && !result.ok) {
        _showError(
          BridgeErrorPresenter.message(
            result,
            fallback: 'Attachment operation failed',
          ),
        );
      }
    });
  }

  void _showError(String text) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
  }

  String _messageStatusLabel(String status) => switch (status) {
    'queued' => 'Queued — waiting for a direct peer connection',
    'sending' => 'Sending…',
    'sent' => 'Sent',
    'delivered' => 'Delivered',
    'read' => 'Read',
    'failed' => 'Delivery failed',
    'cancelled' => 'Cancelled',
    _ => status,
  };
}

bool _sameDay(MessageDto first, MessageDto second) {
  final a = DateTime.fromMillisecondsSinceEpoch(first.createdAtMs).toLocal();
  final b = DateTime.fromMillisecondsSinceEpoch(second.createdAtMs).toLocal();
  return a.year == b.year && a.month == b.month && a.day == b.day;
}

ContactDto? _contactFor(AppSnapshotDto snapshot, ConversationDto conversation) {
  for (final contact in snapshot.contacts) {
    if (contact.id == conversation.contactId) return contact;
  }
  return null;
}

class _DateSeparator extends StatelessWidget {
  const _DateSeparator({required this.milliseconds});
  final int milliseconds;

  @override
  Widget build(BuildContext context) {
    final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    final now = DateTime.now();
    final today = DateTime(now.year, now.month, now.day);
    final day = DateTime(date.year, date.month, date.day);
    final difference = today.difference(day).inDays;
    final label = switch (difference) {
      0 => 'Today',
      1 => 'Yesterday',
      _ =>
        '${date.year}-${date.month.toString().padLeft(2, '0')}-${date.day.toString().padLeft(2, '0')}',
    };
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Center(
        child: Text(label, style: Theme.of(context).textTheme.labelSmall),
      ),
    );
  }
}

class _UnreadSeparator extends StatelessWidget {
  const _UnreadSeparator();

  @override
  Widget build(BuildContext context) => const Row(
    children: <Widget>[
      Expanded(child: Divider()),
      Padding(
        padding: EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        child: Text('New messages'),
      ),
      Expanded(child: Divider()),
    ],
  );
}

class _ReplyComposerPreview extends StatelessWidget {
  const _ReplyComposerPreview({required this.message, required this.onCancel});
  final MessageDto message;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.fromLTRB(12, 8, 4, 8),
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
    ),
    child: Row(
      children: <Widget>[
        Icon(context.torcaIcons.reply, size: 18),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            message.body,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
          ),
        ),
        IconButton(
          tooltip: 'Cancel reply',
          visualDensity: VisualDensity.compact,
          onPressed: onCancel,
          icon: Icon(context.torcaIcons.close),
        ),
      ],
    ),
  );
}

class _PendingAttachment {
  const _PendingAttachment(this.originalName, this.prepared);

  final String originalName;
  final PreparedAttachment prepared;
}

class _AttachmentTray extends StatelessWidget {
  const _AttachmentTray({required this.attachments, required this.onRemove});

  final List<_PendingAttachment> attachments;
  final ValueChanged<_PendingAttachment> onRemove;

  @override
  Widget build(BuildContext context) => Container(
    constraints: const BoxConstraints(minHeight: 72, maxHeight: 116),
    width: double.infinity,
    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      borderRadius: BorderRadius.circular(context.torcaTokens.radiusLarge),
    ),
    child: ListView.separated(
      scrollDirection: Axis.horizontal,
      itemCount: attachments.length,
      separatorBuilder: (_, _) => const SizedBox(width: 8),
      itemBuilder: (context, index) {
        final item = attachments[index];
        final prepared = item.prepared;
        final isImage = prepared.kind == AttachmentMediaKind.image;
        return SizedBox(
          width: 190,
          child: Row(
            children: <Widget>[
              ClipRRect(
                borderRadius: BorderRadius.circular(
                  context.torcaTokens.radiusSmall,
                ),
                child: SizedBox(
                  width: 52,
                  height: 52,
                  child: isImage
                      ? Image.file(File(prepared.path), fit: BoxFit.cover)
                      : ColoredBox(
                          color: Theme.of(context).colorScheme.surface,
                          child: Icon(_iconFor(context, prepared.kind)),
                        ),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      item.originalName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.labelMedium,
                    ),
                    Text(
                      '${prepared.mediaType} · ${formatBytes(prepared.size)}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.labelSmall,
                    ),
                  ],
                ),
              ),
              IconButton(
                tooltip: 'Remove attachment',
                visualDensity: VisualDensity.compact,
                onPressed: () => onRemove(item),
                icon: Icon(context.torcaIcons.close),
              ),
            ],
          ),
        );
      },
    ),
  );

  static IconData _iconFor(BuildContext context, AttachmentMediaKind kind) =>
      switch (kind) {
        AttachmentMediaKind.video => context.torcaIcons.video,
        AttachmentMediaKind.audio => context.torcaIcons.audio,
        AttachmentMediaKind.pdf => context.torcaIcons.pdf,
        AttachmentMediaKind.document => context.torcaIcons.document,
        AttachmentMediaKind.archive => context.torcaIcons.archive,
        AttachmentMediaKind.text => context.torcaIcons.textFile,
        AttachmentMediaKind.image => context.torcaIcons.image,
        AttachmentMediaKind.binary => context.torcaIcons.file,
      };
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}
