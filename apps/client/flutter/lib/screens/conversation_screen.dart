import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:open_filex/open_filex.dart';
import 'package:torca_attachment_processing/torca_attachment_processing.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../platform/platform_capabilities.dart';
import '../platform/video_thumbnail_service.dart';
import '../settings/local_preferences.dart';
import '../widgets/attachment_tile.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/conversation_actions.dart';
import '../widgets/conversation_header.dart';
import '../widgets/message_actions.dart';
import '../widgets/message_bubble.dart';
import '../widgets/operation_tracker.dart';
import '../widgets/radio_conversation_controls.dart';
import '../widgets/voice_clip_recorder.dart';
import '../widgets/voice_message_tile.dart';
import 'connection_details_screen.dart';
import 'conversation_timeline_controller.dart';

part 'conversation_actions.dart';
part 'conversation_formatters.dart';
part 'conversation_widgets.dart';

/// Must stay aligned with `torca_messaging::MessageBody::MAX_CHARACTERS`.
/// The native layer remains authoritative; this value gives immediate UI
/// feedback and avoids queuing a message that cannot be accepted.
const int maxMessageCharacters = 1000;

class ConversationScreen extends StatelessWidget {
  const ConversationScreen({
    required this.gateway,
    required this.conversation,
    this.preferences,
    this.onConversationAction,
    this.conversationPinned = false,
    this.conversationMuted = false,
    super.key,
  });
  final EngineGateway gateway;
  final ConversationDto conversation;
  final LocalPreferences? preferences;
  final Future<void> Function(ConversationAction action)? onConversationAction;
  final bool conversationPinned;
  final bool conversationMuted;

  @override
  Widget build(BuildContext context) => Scaffold(
    body: ConversationPane(
      gateway: gateway,
      conversation: conversation,
      preferences: preferences,
      onConversationAction: onConversationAction,
      conversationPinned: conversationPinned,
      conversationMuted: conversationMuted,
      showHeader: true,
      showBackButton: true,
      headerTopSafeArea: true,
      compactHeader: true,
    ),
  );
}

class ConversationPane extends StatefulWidget {
  const ConversationPane({
    required this.gateway,
    required this.conversation,
    this.preferences,
    this.showHeader = true,
    this.showBackButton = false,
    this.headerTopSafeArea = false,
    this.compactHeader = false,
    this.onConversationAction,
    this.conversationPinned = false,
    this.conversationMuted = false,
    super.key,
  });
  final EngineGateway gateway;
  final ConversationDto conversation;
  final LocalPreferences? preferences;
  final bool showHeader;
  final bool showBackButton;
  final bool headerTopSafeArea;
  final bool compactHeader;
  final Future<void> Function(ConversationAction action)? onConversationAction;
  final bool conversationPinned;
  final bool conversationMuted;

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
  final List<String> _recentEmojis = <String>[];
  final Set<String> _bookmarkedMessageIds = <String>{};
  final Map<String, bool> _pendingReactionStates = <String, bool>{};

  late ConversationTimelineController _timeline;
  Timer? _searchDebounce;
  Timer? _draftDebounce;
  List<MessageDto> _searchResults = const <MessageDto>[];
  bool _searching = false;
  bool _searchBusy = false;
  bool _timelineInitialized = false;
  bool _markingRead = false;
  bool _loadingOlder = false;
  bool _showJumpToLatest = false;
  bool _instantContact = false;
  bool _instantContactBusy = false;
  late bool _conversationPinned;
  late bool _conversationMuted;
  int _jumpMessageCount = 0;
  int _lastActivityAtMs = 0;
  String _lastMessageLifecycleSignature = '';
  String _lastReactionSignature = '';
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
    _conversationPinned = widget.conversationPinned;
    _conversationMuted = widget.conversationMuted;
    _lastActivityAtMs = _conversationSummary()?.lastActivityAtMs ?? 0;
    _lastMessageLifecycleSignature = _messageLifecycleSignature(
      widget.gateway.snapshots.value,
    );
    _lastReactionSignature = _reactionSignature(widget.gateway.snapshots.value);
    unawaited(_initializeTimeline());
    unawaited(_restoreDraft());
    unawaited(_restoreBookmarks());
    unawaited(_restoreInstantContact());
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
    _timelineInitialized = true;
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
      _timelineInitialized = false;
      _lastActivityAtMs = _conversationSummary()?.lastActivityAtMs ?? 0;
      _lastMessageLifecycleSignature = _messageLifecycleSignature(
        widget.gateway.snapshots.value,
      );
      _lastReactionSignature = _reactionSignature(
        widget.gateway.snapshots.value,
      );
      _unreadBoundaryMessageId = null;
      unawaited(_initializeTimeline());
      unawaited(_restoreDraft());
      unawaited(_restoreBookmarks());
    }
    if (oldWidget.conversation.id != widget.conversation.id ||
        oldWidget.conversationPinned != widget.conversationPinned) {
      _conversationPinned = widget.conversationPinned;
    }
    if (oldWidget.conversation.id != widget.conversation.id ||
        oldWidget.conversationMuted != widget.conversationMuted) {
      _conversationMuted = widget.conversationMuted;
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

  Future<void> _restoreBookmarks() async {
    final preferences = widget.preferences;
    if (preferences == null) return;
    final values = await preferences.bookmarkedMessagesFor(
      widget.conversation.id,
    );
    if (!mounted) return;
    setState(() {
      _bookmarkedMessageIds
        ..clear()
        ..addAll(values);
    });
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
    final messageLifecycleSignature = _messageLifecycleSignature(
      widget.gateway.snapshots.value,
    );
    final messageLifecycleChanged =
        messageLifecycleSignature != _lastMessageLifecycleSignature;
    final reactionSignature = _reactionSignature(
      widget.gateway.snapshots.value,
    );
    final reactionsChanged = reactionSignature != _lastReactionSignature;
    _lastActivityAtMs = activity;
    _lastMessageLifecycleSignature = messageLifecycleSignature;
    _lastReactionSignature = reactionSignature;
    if (reactionsChanged && _pendingReactionStates.isNotEmpty) {
      final projected = <String, bool>{};
      for (final reaction in widget.gateway.snapshots.value.reactions) {
        if (reaction.conversationId != widget.conversation.id) continue;
        projected['${reaction.messageId}:${reaction.actorId}:${reaction.emoji}'] =
            reaction.active;
      }
      _pendingReactionStates.removeWhere(
        (key, value) => projected[key] == value,
      );
    }
    // Transport/health revisions are frequent but do not change the
    // conversation projection. Avoid turning every RX/TX LED update into a
    // serialized history query (and let commands overtake stale UI refreshes).
    if (!activityChanged &&
        !messageLifecycleChanged &&
        !reactionsChanged &&
        _timelineInitialized) {
      return;
    }
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

  String _reactionSignature(AppSnapshotDto snapshot) {
    final values =
        snapshot.reactions
            .where(
              (reaction) => reaction.conversationId == widget.conversation.id,
            )
            .map(
              (reaction) =>
                  '${reaction.messageId}:${reaction.actorId}:${reaction.emoji}:${reaction.active}:${reaction.updatedAtMs}',
            )
            .toList()
          ..sort();
    return values.join('|');
  }

  String _messageLifecycleSignature(AppSnapshotDto snapshot) {
    for (final conversation in snapshot.conversations) {
      if (conversation.id == widget.conversation.id) {
        // Root snapshots are overview projections and intentionally contain
        // no message rows. The latest lifecycle is exposed by the summary;
        // use it to trigger a paginated timeline refresh for receipt-only
        // changes that do not alter lastActivityAtMs.
        return '${conversation.lastMessageStatus}:${conversation.unreadCount}';
      }
    }
    return '';
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
            fallback: context.l10n.operationFailed,
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
      final searchQuery = _searchController.text.trim();
      final byId = <String, MessageDto>{
        for (final message in _timeline.messages) message.id: message,
      };
      // Build lookup tables once per snapshot. Filtering the complete
      // attachment/reaction lists for every message made long conversations
      // quadratic and could block the UI during a runtime update or theme
      // rebuild.
      final attachmentsByMessage = <String, List<AttachmentDto>>{};
      for (final attachment in snapshot.attachments) {
        if (!byId.containsKey(attachment.messageId)) continue;
        (attachmentsByMessage[attachment.messageId] ??= <AttachmentDto>[]).add(
          attachment,
        );
      }
      final reactionsByMessage = <String, List<ReactionDto>>{};
      for (final reaction in _timeline.reactions) {
        if (!reaction.active || !byId.containsKey(reaction.messageId)) continue;
        (reactionsByMessage[reaction.messageId] ??= <ReactionDto>[]).add(
          reaction,
        );
      }
      for (final entry in _pendingReactionStates.entries) {
        final parts = entry.key.split(':');
        if (parts.length < 3 || !entry.value) continue;
        final reaction = ReactionDto(
          messageId: parts[0],
          conversationId: widget.conversation.id,
          actorId: parts[1],
          emoji: parts.sublist(2).join(':'),
          active: true,
        );
        final list = reactionsByMessage[reaction.messageId] ??= <ReactionDto>[];
        if (!list.any(
          (value) =>
              value.actorId == reaction.actorId &&
              value.emoji == reaction.emoji,
        )) {
          list.add(reaction);
        }
      }
      final reply = _replyingTo;
      final contact = contactForSnapshot(snapshot, widget.conversation);
      final identityChanged =
          contact?.typedVerificationStatus ==
          VerificationStatus.identityChanged;
      final contactBlocked = contact?.typedStatus == ContactStatus.blocked;
      final composerRestricted = identityChanged || contactBlocked;
      final radioContact = contact == null
          ? null
          : snapshot.radio.forContact(contact.id);
      final radioSession = snapshot.radio.session?.contactId == contact?.id
          ? snapshot.radio.session
          : null;
      final radioState = radioSession?.typedState ?? radioContact?.typedState;
      final radioTimeline = contact == null
          ? const <RadioTimelineEventDto>[]
          : snapshot.radio.timeline
                .where((event) => event.contactId == contact.id)
                .toList(growable: false);
      final sending = _operations.isActive('message:send');
      final sendingAttachment = _operations.isActive('attachment:send');
      final pickingAttachment = _operations.isActive('attachment:pick');
      final transfer = _avatarTransferState(snapshot, widget.conversation.id);

      return ConversationContainer(
        header: widget.showHeader
            ? ConversationHeaderSurface(
                radioActive:
                    radioState == RadioState.transmitting ||
                    radioState == RadioState.receiving,
                topSafeArea: widget.headerTopSafeArea,
                child: ConversationHeader(
                  contact: contact,
                  gateway: widget.gateway,
                  snapshot: snapshot,
                  radio: radioContact,
                  session: radioSession,
                  sending: transfer.$1,
                  receiving: transfer.$2,
                  compact: widget.compactHeader,
                  instantContact: _instantContact,
                  instantContactBusy: _instantContactBusy,
                  radioSupported: capabilitiesFor(widget.gateway).supportsRadio,
                  onInstantContactChanged: _setInstantContact,
                  onConversationActions: widget.onConversationAction == null
                      ? null
                      : () => unawaited(_showConversationActions(snapshot)),
                  leading: widget.showBackButton
                      ? BackButton(
                          onPressed: () => Navigator.of(context).maybePop(),
                        )
                      : null,
                  onConnectionDetails: contact == null
                      ? () {}
                      : () => _openConnectionDetails(contact.id),
                ),
              )
            : null,
        content: Column(
          children: <Widget>[
            if (capabilitiesFor(widget.gateway).supportsRadio &&
                contact != null &&
                (radioContact?.localEnabled == true ||
                    radioTimeline.isNotEmpty))
              RadioConversationStatus(
                contact: contact,
                radio: radioContact,
                session: radioSession,
                timeline: radioTimeline,
                transportFailure:
                    snapshot.radio.lastTransportFailureContactId == contact.id
                    ? snapshot.radio.lastTransportFailure
                    : null,
              ),
            _ConversationSearchBar(
              searching: _searching,
              busy: _searchBusy,
              controller: _searchController,
              onStart: () => setState(() => _searching = true),
              onChanged: _searchChanged,
              onClose: _closeSearch,
            ),
            if (_searching && searchQuery.isNotEmpty)
              Align(
                alignment: Alignment.centerLeft,
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 4),
                  child: Text(
                    context.l10n.searchResultsCount(_searchResults.length),
                    style: Theme.of(context).textTheme.labelMedium,
                  ),
                ),
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
                                    ? context.l10n.typeToSearchConversation
                                    : context.l10n.noMatchingMessages)
                              : '${context.l10n.noMessagesYet}. ${context.l10n.noMessagesYetDescription}',
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
                        final previous = index == 0
                            ? null
                            : messages[index - 1];
                        final showDate =
                            previous == null || !sameDay(previous, message);
                        final showUnread =
                            !_searching &&
                            message.id == _unreadBoundaryMessageId;
                        final grouped =
                            previous != null &&
                            previous.direction == message.direction &&
                            !showDate &&
                            !showUnread &&
                            (message.createdAtMs - previous.createdAtMs).abs() <
                                5 * 60 * 1000;
                        final next = index + 1 < messages.length
                            ? messages[index + 1]
                            : null;
                        final nextShowDate =
                            next != null && !sameDay(message, next);
                        final nextShowUnread =
                            next != null &&
                            !_searching &&
                            next.id == _unreadBoundaryMessageId;
                        final groupedBelow =
                            next != null &&
                            next.direction == message.direction &&
                            !nextShowDate &&
                            !nextShowUnread &&
                            (next.createdAtMs - message.createdAtMs).abs() <
                                5 * 60 * 1000;
                        final quoted = message.replyToMessageId == null
                            ? null
                            : byId[message.replyToMessageId];
                        final attachments =
                            attachmentsByMessage[message.id] ??
                            const <AttachmentDto>[];
                        final retryable =
                            message.typedDirection ==
                                MessageDirection.outbound &&
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
                              reactions:
                                  reactionsByMessage[message.id] ??
                                  const <ReactionDto>[],
                              // Never expose the compatibility announcement
                              // (which contains a path/hash) as chat content.
                              // A typed AttachmentDto is rendered below; until
                              // it arrives, show a safe synchronizing state.
                              showBody:
                                  !attachmentAnnouncement &&
                                  message.typedStatus != MessageStatus.deleted,
                              // Show the identity once at the start of a block.
                              // The compact geometry keeps subsequent messages
                              // visually connected without repeating the name.
                              showSender: !grouped,
                              senderLabel:
                                  message.typedDirection ==
                                      MessageDirection.outbound
                                  ? context.l10n.senderYou
                                  : contact?.displayName ??
                                        context.l10n.contactLabel,
                              senderColorKey:
                                  message.typedDirection ==
                                      MessageDirection.outbound
                                  ? snapshot.identity?.id ?? 'local'
                                  : contact?.remoteIdentityId ??
                                        contact?.id ??
                                        'remote',
                              readByLabel: contact?.displayName,
                              compactTop: grouped,
                              compactBottom: groupedBelow,
                              onLongPress: () => _showMessageActions(message),
                              onSecondaryTapDown: (details) =>
                                  _showMessageActions(
                                    message,
                                    globalPosition: details.globalPosition,
                                  ),
                              quotedBody: message.replyToMessageId == null
                                  ? null
                                  : quoted?.body ??
                                        context.l10n.originalMessageUnavailable,
                              quotedUnavailable:
                                  message.replyToMessageId != null &&
                                  quoted == null,
                              footer: <Widget>[
                                if (_bookmarkedMessageIds.contains(message.id))
                                  Icon(
                                    context.torcaIcons.bookmark,
                                    size: 16,
                                    semanticLabel: context.l10n.bookmarkMessage,
                                  ),
                                if (attachmentAnnouncement &&
                                    attachments.isEmpty)
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
                                            ? context.l10n.retrying
                                            : context.l10n.retryNow,
                                      ),
                                    ),
                                  ),
                                ...attachments.map((attachment) {
                                  final attachmentBusy = _operations
                                      .anyWithPrefix(
                                        'attachment:${attachment.id}:',
                                      );
                                  final retry = () => _attachmentCommand(
                                    attachment.id,
                                    'retry',
                                    RetryAttachmentCommandDto(
                                      attachmentIdHex: attachment.id,
                                    ),
                                  );
                                  final cancel = () => _attachmentCommand(
                                    attachment.id,
                                    'cancel',
                                    CancelAttachmentCommandDto(
                                      attachmentIdHex: attachment.id,
                                    ),
                                  );
                                  if (attachment.mediaType.startsWith(
                                    'audio/',
                                  )) {
                                    return VoiceMessageTile(
                                      attachment: attachment,
                                      operationBusy: attachmentBusy,
                                      materialize: () =>
                                          _materializeAttachment(attachment),
                                      onRetry: retry,
                                      onCancel: cancel,
                                    );
                                  }
                                  return AttachmentTile(
                                    attachment: attachment,
                                    operationBusy: attachmentBusy,
                                    onRetry: retry,
                                    onCancel: cancel,
                                    onOpen: () => _openAttachment(attachment),
                                    onSave: () => _saveAttachment(attachment),
                                    onPreview:
                                        attachment.mediaType.startsWith(
                                          'image/',
                                        )
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
                                        ? () =>
                                              _loadAttachmentPreview(attachment)
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
                          tooltip: context.l10n.jumpToLatest,
                          onPressed: _scrollToBottom,
                          child: Icon(context.torcaIcons.jumpToLatest),
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
        footer: ConversationComposer(
          gateway: widget.gateway,
          messageField: _composerField(
            sending || _searching || composerRestricted,
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
          onInsertEmoji: _insertEmoji,
          onSend: _sendMessage,
          onVoiceClipReady: _queueVoiceClip,
          sending: sending,
          sendingAttachment: sendingAttachment,
          pickingAttachment: pickingAttachment,
          searching: _searching,
          disabled: sending || _searching || composerRestricted,
          disabledMessage: identityChanged
              ? context.l10n.identityChangedSendBlocked
              : contactBlocked
              ? context.l10n.blockedSendBlocked
              : null,
          reply: reply,
          onCancelReply: () => setState(() => _replyingTo = null),
        ),
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
      maxLength: maxMessageCharacters,
      buildCounter:
          (context, {required currentLength, required isFocused, maxLength}) {
            final count = _controller.text.characters.length;
            if (!isFocused &&
                count < (maxLength ?? maxMessageCharacters) * .75) {
              return null;
            }
            final color = count >= (maxLength ?? maxMessageCharacters)
                ? Theme.of(context).colorScheme.error
                : null;
            return Text(
              '$count/${maxLength ?? maxMessageCharacters}',
              style: Theme.of(
                context,
              ).textTheme.labelSmall?.copyWith(color: color),
            );
          },
      textInputAction: TextInputAction.newline,
      decoration: InputDecoration(
        labelText: editing
            ? context.l10n.editMessage
            : replying
            ? context.l10n.reply
            : context.l10n.message,
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

  Future<void> _insertEmoji() async {
    const palette = <String>[
      '\u{1F600}',
      '\u{1F602}',
      '\u{1F44D}',
      '\u{1F44F}',
      '\u{2764}\u{FE0F}',
      '\u{1F389}',
      '\u{1F914}',
      '\u{1F622}',
      '\u{1F525}',
      '\u{1F4AF}',
      '\u{1F64F}',
      '\u{1F60D}',
    ];
    final emoji = await showModalBottomSheet<String>(
      context: context,
      showDragHandle: true,
      builder: (context) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (_recentEmojis.isNotEmpty) ...<Widget>[
                Text(context.l10n.recentEmoji),
                const SizedBox(height: 8),
                _emojiWrap(context, _recentEmojis),
                const SizedBox(height: 12),
              ],
              _emojiWrap(context, palette),
            ],
          ),
        ),
      ),
    );
    if (!mounted || emoji == null) return;
    setState(() {
      _recentEmojis.remove(emoji);
      _recentEmojis.insert(0, emoji);
      if (_recentEmojis.length > 8) _recentEmojis.removeLast();
    });
    final selection = _controller.selection;
    final start = selection.isValid ? selection.start : _controller.text.length;
    final end = selection.isValid ? selection.end : start;
    final value = _controller.text.replaceRange(start, end, emoji);
    final limited = value.characters.take(maxMessageCharacters).toString();
    _controller.value = TextEditingValue(
      text: limited,
      selection: TextSelection.collapsed(
        offset: (start + emoji.length).clamp(0, limited.length).toInt(),
      ),
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

  Future<void> _showConversationActions(AppSnapshotDto snapshot) async {
    final contact = contactForSnapshot(snapshot, widget.conversation);
    final callback = widget.onConversationAction;
    if (contact == null || callback == null) return;
    final summary = _conversationSummary();
    final action = await ConversationActionMenu.showTouch(
      context,
      blocked: contact.typedStatus == ContactStatus.blocked,
      archived: summary?.typedStatus == ConversationStatus.archived,
      pinned: _conversationPinned,
      muted: _conversationMuted,
      unread: (summary?.unreadCount ?? 0) > 0,
    );
    if (!mounted || action == null) return;
    await callback(action);
    if (mounted) {
      if (action == ConversationAction.pinToggle) {
        setState(() => _conversationPinned = !_conversationPinned);
      } else if (action == ConversationAction.muteToggle) {
        setState(() => _conversationMuted = !_conversationMuted);
      }
    }
    if (!mounted || !widget.showBackButton) return;
    if (action == ConversationAction.archive ||
        action == ConversationAction.remove) {
      await Navigator.of(context).maybePop();
    }
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
            fallback: context.l10n.operationFailed,
          ),
        );
      }
    });
  }

  Future<void> _showMessageActions(
    MessageDto message, {
    Offset? globalPosition,
  }) async {
    var quickReactionSelected = false;
    Future<void> applyQuickReaction(String emoji) async {
      quickReactionSelected = true;
      await _reactToMessage(message, emoji: emoji);
    }

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
            bookmarked: _bookmarkedMessageIds.contains(message.id),
            canDelete:
                message.typedDirection == MessageDirection.outbound &&
                message.typedStatus != MessageStatus.cancelled &&
                message.typedStatus != MessageStatus.deleted,
            onQuickReaction: applyQuickReaction,
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
            bookmarked: _bookmarkedMessageIds.contains(message.id),
            canDelete:
                message.typedDirection == MessageDirection.outbound &&
                message.typedStatus != MessageStatus.cancelled &&
                message.typedStatus != MessageStatus.deleted,
            onQuickReaction: applyQuickReaction,
          );
    if (!mounted || action == null) return;
    // The quick-reaction row already submitted the selected emoji. Do not
    // open the full picker a second time for the menu's generic `react` value.
    if (action == MessageAction.react && quickReactionSelected) return;
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
          ScaffoldMessenger.of(
            context,
          ).showSnackBar(SnackBar(content: Text(context.l10n.messageCopied)));
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
      case MessageAction.bookmark:
        setState(() {
          if (!_bookmarkedMessageIds.add(message.id)) {
            _bookmarkedMessageIds.remove(message.id);
          }
        });
        final preferences = widget.preferences;
        if (preferences != null) {
          unawaited(
            preferences.setBookmarkedMessages(
              widget.conversation.id,
              _bookmarkedMessageIds,
            ),
          );
        }
      case MessageAction.cancel:
        final confirmed = await showDialog<bool>(
          context: context,
          builder: (context) => AlertDialog(
            title: Text(context.l10n.cancelMessage),
            content: Text(context.l10n.cancelMessage),
            actions: <Widget>[
              TextButton(
                onPressed: () => Navigator.of(context).pop(false),
                child: Text(context.l10n.close),
              ),
              FilledButton(
                onPressed: () => Navigator.of(context).pop(true),
                child: Text(context.l10n.cancel),
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
              SnackBar(content: Text(context.l10n.messageCancelled)),
            );
          } else {
            _showError(
              BridgeErrorPresenter.localized(
                context,
                result,
                fallback: context.l10n.operationFailed,
              ),
            );
          }
        });
      case MessageAction.delete:
        final confirmed = await showDialog<bool>(
          context: context,
          builder: (context) => AlertDialog(
            title: Text(context.l10n.deleteMessageTitle),
            content: Text(context.l10n.deleteMessage),
            actions: <Widget>[
              TextButton(
                onPressed: () => Navigator.of(context).pop(false),
                child: Text(context.l10n.close),
              ),
              FilledButton(
                onPressed: () => Navigator.of(context).pop(true),
                child: Text(context.l10n.deleteMessage),
              ),
            ],
          ),
        );
        if (confirmed != true) return;
        BridgeResultDto result;
        try {
          result = await widget.gateway.execute(
            DeleteMessageCommandDto(messageIdHex: message.id),
          );
        } on Object {
          if (mounted) _showError(context.l10n.operationFailed);
          return;
        }
        if (!result.ok && mounted) {
          _showError(
            BridgeErrorPresenter.localized(
              context,
              result,
              fallback: context.l10n.operationFailed,
            ),
          );
        }
      case MessageAction.details:
        await _showMessageDetails(message);
    }
  }

  Future<void> _reactToMessage(MessageDto message, {String? emoji}) async {
    final actorId = widget.gateway.snapshots.value.identity?.id;
    if (actorId == null || actorId.isEmpty) {
      _showError(context.l10n.localIdentityNotReady);
      return;
    }
    final selectedEmoji =
        emoji ??
        await showModalBottomSheet<String>(
          context: context,
          builder: (context) => SafeArea(
            child: Wrap(
              alignment: WrapAlignment.center,
              children: <Widget>[
                for (final value in MessageActionMenu.quickReactions)
                  IconButton(
                    icon: Text(value, style: const TextStyle(fontSize: 26)),
                    onPressed: () => Navigator.of(context).pop(value),
                    tooltip: value,
                  ),
              ],
            ),
          ),
        );
    if (!mounted || selectedEmoji == null) return;
    final existing = _timeline.reactions.any(
      (reaction) =>
          reaction.messageId == message.id &&
          reaction.actorId == actorId &&
          reaction.emoji == selectedEmoji &&
          reaction.active,
    );
    final key = '${message.id}:$actorId:$selectedEmoji';
    final nextActive = !(existing || _pendingReactionStates[key] == true);
    setState(() => _pendingReactionStates[key] = nextActive);
    BridgeResultDto result;
    try {
      result = await widget.gateway.execute(
        SetMessageReactionCommandDto(
          messageIdHex: message.id,
          conversationIdHex: message.conversationId,
          actorIdHex: actorId,
          emoji: selectedEmoji,
          active: nextActive,
        ),
      );
    } on Object {
      if (mounted) setState(() => _pendingReactionStates.remove(key));
      if (mounted) _showError(context.l10n.couldNotUpdateReaction);
      return;
    }
    if (!result.ok && mounted) {
      setState(() => _pendingReactionStates.remove(key));
      _showError(
        BridgeErrorPresenter.localized(
          context,
          result,
          fallback: context.l10n.couldNotUpdateReaction,
        ),
      );
    }
  }

  Future<void> _showMessageDetails(MessageDto message) => showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(context.l10n.messageDetails),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          _detail('ID', message.id),
          _detail('Direction', message.direction),
          _detail('Status', messageStatusLabel(message.status, context.l10n)),
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
          child: Text(context.l10n.close),
        ),
      ],
    ),
  );
}

(bool, bool) _avatarTransferState(
  AppSnapshotDto snapshot,
  String conversationId,
) {
  final messageIds = snapshot.messages
      .where((message) => message.conversationId == conversationId)
      .map((message) => message.id)
      .toSet();
  var sending = false;
  var receiving = false;
  for (final attachment in snapshot.attachments) {
    if (!messageIds.contains(attachment.messageId) ||
        attachment.typedStatus != AttachmentStatus.transferring) {
      continue;
    }
    if (attachment.typedDirection == AttachmentDirection.outbound) {
      sending = true;
    } else if (attachment.typedDirection == AttachmentDirection.inbound) {
      receiving = true;
    }
  }
  return (sending, receiving);
}
