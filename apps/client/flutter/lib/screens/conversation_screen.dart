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
import '../localization/torca_strings.dart';
import '../platform/platform_capabilities.dart';
import '../platform/video_thumbnail_service.dart';
import '../settings/local_preferences.dart';
import '../widgets/attachment_tile.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/conversation_header.dart';
import '../widgets/message_actions.dart';
import '../widgets/message_bubble.dart';
import '../widgets/operation_tracker.dart';
import '../widgets/radio_conversation_controls.dart';
import '../widgets/runtime_network_status.dart';
import 'connection_details_screen.dart';
import 'conversation_timeline_controller.dart';

part 'conversation_widgets.dart';
part 'conversation_formatters.dart';

class ConversationScreen extends StatelessWidget {
  const ConversationScreen({
    required this.gateway,
    required this.conversation,
    this.preferences,
    super.key,
  });
  final EngineGateway gateway;
  final ConversationDto conversation;
  final LocalPreferences? preferences;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
    valueListenable: gateway.snapshots,
    builder: (context, snapshot, _) {
      final contact = contactForSnapshot(snapshot, conversation);
      final radioSession = snapshot.radio.session?.contactId == contact?.id
          ? snapshot.radio.session
          : null;
      final recording = radioSession?.typedState == RadioState.transmitting;
      return Scaffold(
        appBar: RuntimeAppBar(
          titleSpacing: 0,
          backgroundColor: recording
              ? Theme.of(context).colorScheme.error.withValues(alpha: 0.10)
              : null,
          title: ConversationHeader(
            contact: contact,
            gateway: gateway,
            radio: contact == null
                ? null
                : snapshot.radio.forContact(contact.id),
            session: radioSession,
            compact: true,
            onConnectionDetails: contact == null
                ? () {}
                : () => _openConnectionDetails(context, contact.id),
          ),
        ),
        body: ConversationPane(
          gateway: gateway,
          conversation: conversation,
          preferences: preferences,
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
    this.preferences,
    this.showHeader = true,
    super.key,
  });
  final EngineGateway gateway;
  final ConversationDto conversation;
  final LocalPreferences? preferences;
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
  Timer? _draftDebounce;
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
  MessageDto? _editingMessage;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _timeline = _newTimeline();
    _timeline.addListener(_timelineChanged);
    _operations.addListener(_operationChanged);
    _scrollController.addListener(_scrollChanged);
    _controller.addListener(_draftChanged);
    widget.gateway.snapshots.addListener(_snapshotChanged);
    _lastActivityAtMs = _conversationSummary()?.lastActivityAtMs ?? 0;
    unawaited(_initializeTimeline());
    unawaited(_restoreDraft());
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
      _saveDraft(oldWidget.conversation.id);
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
      unawaited(_restoreDraft());
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
    _draftDebounce?.cancel();
    _saveDraft(widget.conversation.id);
    _searchController.dispose();
    _scrollController.removeListener(_scrollChanged);
    _operations.removeListener(_operationChanged);
    _operations.dispose();
    _scrollController.dispose();
    _controller.removeListener(_draftChanged);
    _controller.dispose();
    super.dispose();
  }

  Future<void> _restoreDraft() async {
    final preferences = widget.preferences;
    if (preferences == null) return;
    final value = await preferences.draftFor(widget.conversation.id);
    if (!mounted ||
        value == null ||
        value.isEmpty ||
        _controller.text.isNotEmpty) {
      return;
    }
    _controller.value = TextEditingValue(
      text: value,
      selection: TextSelection.collapsed(offset: value.length),
    );
  }

  void _draftChanged() {
    final preferences = widget.preferences;
    if (preferences == null) return;
    _draftDebounce?.cancel();
    _draftDebounce = Timer(const Duration(milliseconds: 300), () {
      _saveDraft(widget.conversation.id);
    });
  }

  void _saveDraft(String conversationId) {
    final preferences = widget.preferences;
    if (preferences == null) return;
    final value = conversationId == widget.conversation.id
        ? _controller.text
        : _drafts[conversationId] ?? '';
    unawaited(
      value.trim().isEmpty
          ? preferences.clearDraft(conversationId)
          : preferences.setDraft(conversationId, value),
    );
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
          BridgeErrorPresenter.localized(
            context,
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
      final contact = contactForSnapshot(snapshot, widget.conversation);
      final radioContact = contact == null
          ? null
          : snapshot.radio.forContact(contact.id);
      final radioSession = snapshot.radio.session?.contactId == contact?.id
          ? snapshot.radio.session
          : null;
      final radioTimeline = contact == null
          ? const <RadioTimelineEventDto>[]
          : snapshot.radio.timeline
                .where((event) => event.contactId == contact.id)
                .toList(growable: false);
      final sending = _operations.isActive('message:send');
      final sendingAttachment = _operations.isActive('attachment:send');
      final pickingAttachment = _operations.isActive('attachment:pick');

      return Column(
        children: <Widget>[
          if (widget.showHeader) ...<Widget>[
            ColoredBox(
              color: radioSession?.typedState == RadioState.transmitting
                  ? Theme.of(context).colorScheme.error.withValues(alpha: 0.10)
                  : Colors.transparent,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 10, 8, 10),
                child: ConversationHeader(
                  contact: contact,
                  gateway: widget.gateway,
                  radio: radioContact,
                  session: radioSession,
                  onConnectionDetails: contact == null
                      ? () {}
                      : () => _openConnectionDetails(contact.id),
                ),
              ),
            ),
            const Divider(height: 1),
          ],
          if (contact != null &&
              (radioContact?.localEnabled == true || radioTimeline.isNotEmpty))
            RadioConversationStatus(
              contact: contact,
              radio: radioContact,
              session: radioSession,
              timeline: radioTimeline,
            ),
          _ConversationSearchBar(
            searching: _searching,
            busy: _searchBusy,
            controller: _searchController,
            onStart: () => setState(() => _searching = true),
            onChanged: _searchChanged,
            onClose: _closeSearch,
          ),
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
                            : '${context.strings.noMessagesYet}. ${context.strings.noMessagesYetDescription}',
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
                          previous == null || !sameDay(previous, message);
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
                            reactions: snapshot.reactions
                                .where(
                                  (reaction) =>
                                      reaction.messageId == message.id,
                                )
                                .toList(growable: false),
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
                                AttachmentPendingTile(
                                  name: message.body.substring(
                                    'Attachment: '.length,
                                  ),
                                  outbound:
                                      message.typedDirection ==
                                      MessageDirection.outbound,
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
                                      retryBusy
                                          ? context.strings.retrying
                                          : context.strings.retryNow,
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
                                  // A v2 preview is a small, separately
                                  // encrypted JPEG.  It is available before
                                  // the complete attachment, for both images
                                  // and videos.  Images open in-app; a video
                                  // card uses the same cover image but opens
                                  // the final media in the platform player.
                                  onPreview:
                                      attachment.mediaType.startsWith('image/')
                                      ? () => _previewAttachment(attachment)
                                      : attachment.mediaType.startsWith(
                                          'video/',
                                        )
                                      ? () => _openAttachment(attachment)
                                      : null,
                                  loadPreview:
                                      hasVisualAttachmentPreview(
                                        attachment.mediaType,
                                      )
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
                        tooltip: context.strings.jumpToLatest,
                        onPressed: _scrollToBottom,
                        child: Icon(context.torcaIcons.jumpToLatest),
                      ),
                    ),
                  ),
              ],
            ),
          ),
          const Divider(height: 1),
          ConversationComposer(
            gateway: widget.gateway,
            messageField: _composerField(
              sending || _searching,
              reply != null,
              _editingMessage != null,
            ),
            contact: contact,
            radio: radioContact,
            session: radioSession,
            pendingAttachments: _pendingAttachments,
            onRemoveAttachment: (pending) => setState(() {
              _pendingAttachments.remove(pending);
              unawaited(pending.prepared.dispose());
            }),
            onPickAttachments: _pickAttachments,
            onSend: _sendMessage,
            sending: sending,
            sendingAttachment: sendingAttachment,
            pickingAttachment: pickingAttachment,
            searching: _searching,
            reply: reply,
            onCancelReply: () => setState(() => _replyingTo = null),
          ),
        ],
      );
    },
  );

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

  Widget _composerField(bool disabled, bool replying, bool editing) {
    final field = TextField(
      controller: _controller,
      enabled: !disabled,
      minLines: 1,
      maxLines: 5,
      textInputAction: TextInputAction.newline,
      decoration: InputDecoration(
        labelText: editing
            ? context.strings.editMessage
            : replying
            ? 'Reply'
            : 'Message',
      ),
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
          BridgeErrorPresenter.localized(
            context,
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
        ? await MessageActionMenu.showTouch(
            context,
            canCancel:
                message.typedDirection == MessageDirection.outbound &&
                (message.typedStatus == MessageStatus.queued ||
                    message.typedStatus == MessageStatus.failed),
            canEdit:
                message.typedDirection == MessageDirection.outbound &&
                (message.typedStatus == MessageStatus.queued ||
                    message.typedStatus == MessageStatus.failed),
          )
        : await MessageActionMenu.showDesktop(
            context,
            globalPosition,
            canCancel:
                message.typedDirection == MessageDirection.outbound &&
                (message.typedStatus == MessageStatus.queued ||
                    message.typedStatus == MessageStatus.failed),
            canEdit:
                message.typedDirection == MessageDirection.outbound &&
                (message.typedStatus == MessageStatus.queued ||
                    message.typedStatus == MessageStatus.failed),
          );
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
      case MessageAction.react:
        await _reactToMessage(message);
      case MessageAction.copy:
        await Clipboard.setData(ClipboardData(text: message.body));
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(context.strings.messageCopied)),
          );
        }
      case MessageAction.edit:
        if (!_searching) {
          setState(() {
            _editingMessage = message;
            _replyingTo = null;
            _controller
              ..text = message.body
              ..selection = TextSelection.collapsed(
                offset: message.body.length,
              );
          });
        }
      case MessageAction.forward:
        await _forwardMessage(message);
      case MessageAction.cancel:
        final confirmed = await showDialog<bool>(
          context: context,
          builder: (context) => AlertDialog(
            title: Text(context.strings.cancelMessage),
            content: Text(context.strings.cancelMessage),
            actions: <Widget>[
              TextButton(
                onPressed: () => Navigator.of(context).pop(false),
                child: Text(context.strings.close),
              ),
              FilledButton(
                onPressed: () => Navigator.of(context).pop(true),
                child: Text(context.strings.cancel),
              ),
            ],
          ),
        );
        if (confirmed != true) return;
        await _operations.run('message:${message.id}:cancel', () async {
          final result = await widget.gateway.execute(
            CancelMessageCommandDto(messageIdHex: message.id),
          );
          if (!mounted) return;
          if (result.ok) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text(context.strings.messageCancelled)),
            );
          } else {
            _showError(
              BridgeErrorPresenter.localized(
                context,
                result,
                fallback: 'Could not cancel message',
              ),
            );
          }
        });
      case MessageAction.details:
        await _showMessageDetails(message);
    }
  }

  Future<void> _reactToMessage(MessageDto message) async {
    final actorId = widget.gateway.snapshots.value.identity?.id;
    if (actorId == null || actorId.isEmpty) {
      _showError('Local identity is not ready');
      return;
    }
    final emoji = await showModalBottomSheet<String>(
      context: context,
      builder: (context) => SafeArea(
        child: Wrap(
          alignment: WrapAlignment.center,
          children: <Widget>[
            for (final value in const <String>[
              '👍',
              '❤️',
              '😂',
              '😮',
              '😢',
              '👏',
            ])
              IconButton(
                icon: Text(value, style: const TextStyle(fontSize: 26)),
                onPressed: () => Navigator.of(context).pop(value),
                tooltip: value,
              ),
          ],
        ),
      ),
    );
    if (!mounted || emoji == null) return;
    final existing = widget.gateway.snapshots.value.reactions.any(
      (reaction) =>
          reaction.messageId == message.id &&
          reaction.actorId == actorId &&
          reaction.emoji == emoji &&
          reaction.active,
    );
    await widget.gateway.execute(
      SetMessageReactionCommandDto(
        messageIdHex: message.id,
        conversationIdHex: message.conversationId,
        actorIdHex: actorId,
        emoji: emoji,
        active: !existing,
      ),
    );
  }

  Future<void> _showMessageDetails(MessageDto message) => showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(context.strings.messageDetails),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          _detail('ID', message.id),
          _detail('Direction', message.direction),
          _detail(
            'Status',
            messageStatusLabel(message.status, context.strings),
          ),
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
          child: Text(context.strings.close),
        ),
      ],
    ),
  );

  Future<void> _forwardMessage(MessageDto message) async {
    final snapshot = widget.gateway.snapshots.value;
    final options = snapshot.conversations
        .where((conversation) => conversation.id != widget.conversation.id)
        .toList(growable: false);
    if (!mounted || options.isEmpty) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(context.strings.chooseConversation)),
        );
      }
      return;
    }
    final target = await showDialog<ConversationDto>(
      context: context,
      builder: (context) => SimpleDialog(
        title: Text(context.strings.chooseConversation),
        children: options
            .map(
              (conversation) => SimpleDialogOption(
                onPressed: () => Navigator.of(context).pop(conversation),
                child: Text(
                  contactForSnapshot(snapshot, conversation)?.displayName ??
                      context.strings.contactLabel,
                ),
              ),
            )
            .toList(growable: false),
      ),
    );
    if (!mounted || target == null) return;
    final result = await widget.gateway.execute(
      QueueMessageCommandDto(conversationIdHex: target.id, body: message.body),
    );
    if (!mounted) return;
    if (!result.ok) {
      _showError(
        BridgeErrorPresenter.localized(
          context,
          result,
          fallback: 'Could not forward message',
        ),
      );
    }
  }

  Widget _detail(String label, String value) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Text('$label: $value'),
  );

  String _date(int ms) => ms <= 0
      ? context.strings.unavailable
      : DateTime.fromMillisecondsSinceEpoch(ms).toLocal().toString();

  Future<void> _sendMessage() async {
    final body = _controller.text.trim();
    if ((body.isEmpty && _pendingAttachments.isEmpty) || _searching) return;
    if (body.isNotEmpty) {
      final editing = _editingMessage;
      final replyTo = _replyingTo?.id;
      var sent = false;
      await _operations.run('message:send', () async {
        final result = await widget.gateway.execute(
          editing == null
              ? QueueMessageCommandDto(
                  conversationIdHex: widget.conversation.id,
                  body: body,
                  replyToMessageId: replyTo,
                )
              : EditMessageCommandDto(messageIdHex: editing.id, body: body),
        );
        if (!mounted) return;
        if (result.ok) {
          sent = true;
          _controller.clear();
          _drafts.remove(widget.conversation.id);
          final preferences = widget.preferences;
          if (preferences != null) {
            unawaited(preferences.clearDraft(widget.conversation.id));
          }
          setState(() {
            _replyingTo = null;
            _editingMessage = null;
          });
          await _timeline.refreshLatest();
          WidgetsBinding.instance.addPostFrameCallback(
            (_) => _scrollToBottom(),
          );
        } else {
          _showError(
            BridgeErrorPresenter.localized(
              context,
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
            videoPreviewExtractor: VideoThumbnailService.extract,
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
            previewSourcePath: prepared.previewPath,
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
            '${item.originalName}: ${BridgeErrorPresenter.localized(context, response, fallback: context.strings.couldNotQueueAttachment)}',
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
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(context.strings.attachmentsQueued(queued))),
        );
      }
    });
  }

  Future<void> _saveAttachment(AttachmentDto attachment) async {
    await _operations.run('attachment:${attachment.id}:save', () async {
      final path = await FilePicker.saveFile(
        dialogTitle: context.strings.saveAttachment,
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
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(context.strings.attachmentSaved)),
        );
      } else {
        _showError(
          BridgeErrorPresenter.localized(
            context,
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
    final preview = await widget.gateway.execute(
      ExportAttachmentPreviewCommandDto(
        attachmentIdHex: attachment.id,
        destinationPath: path,
      ),
    );
    if (preview.ok && await file.exists()) return path;
    // Attachments created by an older peer/build have no v2 preview.  A fully
    // available image remains previewable via the original payload.
    if (attachment.typedStatus != AttachmentStatus.available) return null;
    final result = await widget.gateway.execute(
      ExportAttachmentCommandDto(
        attachmentIdHex: attachment.id,
        destinationPath: path,
      ),
    );
    return result.ok && await file.exists() ? path : null;
  }

  Future<void> _previewAttachment(AttachmentDto attachment) async {
    final path = await _loadAttachmentPreview(attachment);
    if (!mounted || path == null) {
      if (mounted) _showError('Could not load image preview');
      return;
    }
    await showDialog<void>(
      context: context,
      builder: (context) => Dialog(
        clipBehavior: Clip.antiAlias,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 960, maxHeight: 760),
          child: Stack(
            children: <Widget>[
              Positioned.fill(
                child: InteractiveViewer(
                  minScale: 0.5,
                  maxScale: 5,
                  child: Center(child: Image.file(File(path))),
                ),
              ),
              Positioned(
                top: 8,
                right: 8,
                child: IconButton.filledTonal(
                  tooltip: context.strings.close,
                  onPressed: () => Navigator.of(context).pop(),
                  icon: Icon(context.torcaIcons.close),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _openAttachment(AttachmentDto attachment) async {
    await _operations.run('attachment:${attachment.id}:open', () async {
      // The display name intentionally stays faithful to the user's selected
      // filename even when an image was recompressed.  The temporary file used
      // for the platform opener must instead use the authoritative MIME type
      // so an optimised PNG (now JPEG bytes) is opened correctly.
      final ext =
          contentExtension(attachment.mediaType) ??
          safeExtension(attachment.name);
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
          BridgeErrorPresenter.localized(
            context,
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

  Future<void> _attachmentCommand(
    String attachmentId,
    String action,
    BridgeCommandDto command,
  ) async {
    await _operations.run('attachment:$attachmentId:$action', () async {
      final result = await widget.gateway.execute(command);
      if (mounted && !result.ok) {
        _showError(
          BridgeErrorPresenter.localized(
            context,
            result,
            fallback: context.strings.attachmentOperationFailed,
          ),
        );
      }
    });
  }

  void _showError(String text) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
  }
}
