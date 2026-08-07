import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:open_filex/open_filex.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/attachment_tile.dart';
import '../widgets/bridge_error_presenter.dart';
import '../widgets/conversation_header.dart';
import '../widgets/message_actions.dart';
import '../widgets/message_bubble.dart';
import '../widgets/operation_tracker.dart';
import 'connection_details_screen.dart';

class ConversationScreen extends StatelessWidget {
  const ConversationScreen({required this.gateway, required this.conversation, super.key});
  final EngineGateway gateway;
  final ConversationDto conversation;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
        valueListenable: gateway.snapshots,
        builder: (context, snapshot, _) {
          final contact = _contactFor(snapshot, conversation);
          return Scaffold(
            appBar: AppBar(
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
        builder: (_) => ConnectionDetailsScreen(gateway: gateway, contactId: contactId),
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

class _ConversationPaneState extends State<ConversationPane> {
  final TextEditingController _controller = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  final OperationTracker _operations = OperationTracker();
  final Map<String, String> _drafts = <String, String>{};
  bool _markingRead = false;
  bool _showJumpToLatest = false;
  int _lastMessageCount = 0;
  String? _unreadBoundaryMessageId;
  MessageDto? _replyingTo;

  @override
  void initState() {
    super.initState();
    _operations.addListener(_operationChanged);
    _scrollController.addListener(_scrollChanged);
    widget.gateway.snapshots.addListener(_snapshotChanged);
    _captureConversationState(widget.conversation.id);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      unawaited(_markReadIfNeeded());
      _scrollToBottom(jump: true);
    });
  }

  @override
  void didUpdateWidget(covariant ConversationPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.gateway != widget.gateway) {
      oldWidget.gateway.snapshots.removeListener(_snapshotChanged);
      widget.gateway.snapshots.addListener(_snapshotChanged);
    }
    if (oldWidget.conversation.id != widget.conversation.id) {
      _drafts[oldWidget.conversation.id] = _controller.text;
      _controller.text = _drafts[widget.conversation.id] ?? '';
      _replyingTo = null;
      _showJumpToLatest = false;
      _captureConversationState(widget.conversation.id);
      unawaited(_markReadIfNeeded());
      WidgetsBinding.instance.addPostFrameCallback((_) => _scrollToBottom(jump: true));
    }
  }

  @override
  void dispose() {
    widget.gateway.snapshots.removeListener(_snapshotChanged);
    _scrollController.removeListener(_scrollChanged);
    _operations.removeListener(_operationChanged);
    _operations.dispose();
    _scrollController.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _captureConversationState(String conversationId) {
    final messages = _messagesFor(widget.gateway.snapshots.value, conversationId);
    _lastMessageCount = messages.length;
    _unreadBoundaryMessageId = messages
        .where((message) => message.direction == 'inbound' && message.status == 'delivered')
        .map((message) => message.id)
        .firstOrNull;
  }

  void _operationChanged() {
    if (mounted) setState(() {});
  }

  void _scrollChanged() {
    if (!_showJumpToLatest || !_nearBottom()) return;
    setState(() => _showJumpToLatest = false);
  }

  void _snapshotChanged() {
    unawaited(_markReadIfNeeded());
    final count = _messagesFor(
      widget.gateway.snapshots.value,
      widget.conversation.id,
    ).length;
    if (count <= _lastMessageCount) {
      _lastMessageCount = count;
      return;
    }
    final follow = _nearBottom();
    _lastMessageCount = count;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      if (follow) {
        _scrollToBottom();
      } else if (!_showJumpToLatest) {
        setState(() => _showJumpToLatest = true);
      }
    });
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
    if (_showJumpToLatest && mounted) setState(() => _showJumpToLatest = false);
  }

  Future<void> _markReadIfNeeded() async {
    if (_markingRead) return;
    final hasDelivered = widget.gateway.snapshots.value.messages.any(
      (message) =>
          message.conversationId == widget.conversation.id &&
          message.direction == 'inbound' &&
          message.status == 'delivered',
    );
    if (!hasDelivered) return;
    _markingRead = true;
    try {
      await widget.gateway.execute(
        MarkConversationReadCommandDto(conversationIdHex: widget.conversation.id),
      );
    } finally {
      _markingRead = false;
    }
  }

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<AppSnapshotDto>(
        valueListenable: widget.gateway.snapshots,
        builder: (context, snapshot, _) {
          final messages = _messagesFor(snapshot, widget.conversation.id);
          final byId = <String, MessageDto>{for (final message in messages) message.id: message};
          final reply = _replyingTo;
          final contact = _contactFor(snapshot, widget.conversation);
          final sending = _operations.isActive('message:send');
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
              Expanded(
                child: Stack(
                  children: <Widget>[
                    if (messages.isEmpty)
                      const Center(
                        child: Padding(
                          padding: EdgeInsets.all(24),
                          child: Text(
                            'No messages yet. Messages are sent directly through Tor.',
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
                          final showDate = previous == null || !_sameDay(previous, message);
                          final showUnread = message.id == _unreadBoundaryMessageId;
                          final grouped = previous != null &&
                              previous.direction == message.direction &&
                              !showDate &&
                              !showUnread &&
                              (message.createdAtMs - previous.createdAtMs).abs() < 5 * 60 * 1000;
                          final quoted = message.replyToMessageId == null
                              ? null
                              : byId[message.replyToMessageId];
                          final attachments = snapshot.attachments
                              .where((attachment) => attachment.messageId == message.id)
                              .toList(growable: false);
                          final retryable =
                              message.direction == 'outbound' && message.status == 'failed';
                          final retryBusy = _operations.isActive('message:${message.id}:retry');

                          return Column(
                            children: <Widget>[
                              if (showDate) _DateSeparator(milliseconds: message.createdAtMs),
                              if (showUnread) const _UnreadSeparator(),
                              MessageBubble(
                                message: message,
                                compactTop: grouped,
                                onLongPress: () => _showMessageActions(message),
                                onSecondaryTapDown: (details) => _showMessageActions(
                                  message,
                                  globalPosition: details.globalPosition,
                                ),
                                quotedBody: message.replyToMessageId == null
                                    ? null
                                    : quoted?.body ?? 'Original message unavailable',
                                quotedUnavailable:
                                    message.replyToMessageId != null && quoted == null,
                                footer: <Widget>[
                                  if (retryable)
                                    Align(
                                      alignment: Alignment.centerLeft,
                                      child: TextButton.icon(
                                        onPressed: retryBusy ? null : () => _retryMessage(message),
                                        icon: retryBusy
                                            ? const SizedBox(
                                                width: 16,
                                                height: 16,
                                                child: CircularProgressIndicator(strokeWidth: 2),
                                              )
                                            : const Icon(Icons.refresh),
                                        label: Text(retryBusy ? 'Retrying…' : 'Retry now'),
                                      ),
                                    ),
                                  ...attachments.map((attachment) {
                                    final attachmentBusy = _operations.anyWithPrefix(
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
                                    );
                                  }),
                                ],
                              ),
                            ],
                          );
                        },
                      ),
                    if (_showJumpToLatest)
                      Positioned(
                        right: 16,
                        bottom: 12,
                        child: FloatingActionButton.small(
                          heroTag: null,
                          tooltip: 'Jump to latest message',
                          onPressed: _scrollToBottom,
                          child: const Icon(Icons.arrow_downward),
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
                            onPressed: pickingAttachment ? null : _pickAttachments,
                            icon: pickingAttachment
                                ? const SizedBox(
                                    width: 20,
                                    height: 20,
                                    child: CircularProgressIndicator(strokeWidth: 2),
                                  )
                                : const Icon(Icons.attach_file),
                          ),
                          const SizedBox(width: 4),
                          Expanded(child: _composerField(sending, reply != null)),
                          const SizedBox(width: 8),
                          IconButton.filled(
                            tooltip: sending ? 'Sending message' : 'Send message',
                            onPressed: sending ? null : _sendMessage,
                            icon: sending
                                ? const SizedBox(
                                    width: 18,
                                    height: 18,
                                    child: CircularProgressIndicator(strokeWidth: 2),
                                  )
                                : const Icon(Icons.send),
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

  Widget _composerField(bool sending, bool replying) {
    final field = TextField(
      controller: _controller,
      enabled: !sending,
      minLines: 1,
      maxLines: 5,
      textInputAction: TextInputAction.newline,
      decoration: InputDecoration(labelText: replying ? 'Reply' : 'Message'),
    );
    if (!(Platform.isWindows || Platform.isLinux || Platform.isMacOS)) return field;
    return CallbackShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.enter): _sendMessage,
        const SingleActivator(LogicalKeyboardKey.enter, shift: true): _insertNewline,
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
        _showError(BridgeErrorPresenter.message(result, fallback: 'Could not retry message'));
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

  Future<void> _applyMessageAction(MessageDto message, MessageAction action) async {
    switch (action) {
      case MessageAction.reply:
        setState(() => _replyingTo = message);
      case MessageAction.copy:
        await Clipboard.setData(ClipboardData(text: message.body));
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('Message copied')),
          );
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
              _detail('Last update', _date(message.updatedAtMs)),
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
    if (body.isEmpty) return;
    final replyTo = _replyingTo?.id;
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
        _controller.clear();
        _drafts.remove(widget.conversation.id);
        setState(() => _replyingTo = null);
        WidgetsBinding.instance.addPostFrameCallback((_) => _scrollToBottom());
      } else {
        _showError(BridgeErrorPresenter.message(result, fallback: 'Could not queue message'));
      }
    });
  }

  Future<void> _pickAttachments() async {
    await _operations.run('attachment:pick', () async {
      final picked = await FilePicker.platform.pickFiles(
        allowMultiple: true,
        withData: false,
      );
      if (picked == null || picked.files.isEmpty || !mounted) return;
      final maxBytes = capabilitiesFor(widget.gateway).maxAttachmentBytes;
      var queued = 0;
      for (final file in picked.files) {
        final path = file.path;
        if (path == null || path.isEmpty) {
          _showError('${file.name}: local file path is unavailable');
          continue;
        }
        if (file.size <= 0 || file.size > maxBytes) {
          _showError('${file.name}: maximum attachment size is ${formatBytes(maxBytes)}');
          continue;
        }
        final response = await widget.gateway.execute(
          QueueAttachmentCommandDto(
            conversationIdHex: widget.conversation.id,
            sourcePath: path,
            name: file.name,
            mediaType: _mediaType(file.extension),
            size: file.size,
          ),
        );
        if (!mounted) return;
        if (!response.ok) {
          _showError(
            '${file.name}: ${BridgeErrorPresenter.message(response, fallback: 'Could not queue attachment')}',
          );
          continue;
        }
        queued++;
      }
      if (mounted && queued > 1) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('$queued attachments queued')),
        );
      }
    });
  }

  Future<void> _saveAttachment(AttachmentDto attachment) async {
    await _operations.run('attachment:${attachment.id}:save', () async {
      final path = await FilePicker.platform.saveFile(
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
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Attachment saved')),
        );
      } else {
        _showError(BridgeErrorPresenter.message(result, fallback: 'Could not save attachment'));
      }
    });
  }

  Future<void> _openAttachment(AttachmentDto attachment) async {
    await _operations.run('attachment:${attachment.id}:open', () async {
      final ext = _safeExtension(attachment.name);
      final path =
          '${Directory.systemTemp.path}${Platform.pathSeparator}torca-${attachment.id}$ext';
      final result = await widget.gateway.execute(
        ExportAttachmentCommandDto(
          attachmentIdHex: attachment.id,
          destinationPath: path,
        ),
      );
      if (!mounted) return;
      if (!result.ok) {
        _showError(BridgeErrorPresenter.message(result, fallback: 'Could not open attachment'));
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
        _showError(BridgeErrorPresenter.message(result, fallback: 'Attachment operation failed'));
      }
    });
  }

  void _showError(String text) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
  }

  String _mediaType(String? extension) {
    switch ((extension ?? '').toLowerCase()) {
      case 'jpg':
      case 'jpeg':
        return 'image/jpeg';
      case 'png':
        return 'image/png';
      case 'gif':
        return 'image/gif';
      case 'webp':
        return 'image/webp';
      case 'pdf':
        return 'application/pdf';
      case 'txt':
        return 'text/plain';
      case 'mp4':
        return 'video/mp4';
      case 'mp3':
        return 'audio/mpeg';
      default:
        return 'application/octet-stream';
    }
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

List<MessageDto> _messagesFor(AppSnapshotDto snapshot, String conversationId) => snapshot.messages
    .where((message) => message.conversationId == conversationId)
    .toList(growable: false);

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
      _ => '${date.year}-${date.month.toString().padLeft(2, '0')}-${date.day.toString().padLeft(2, '0')}',
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
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          children: <Widget>[
            const Icon(Icons.reply, size: 18),
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
              icon: const Icon(Icons.close),
            ),
          ],
        ),
      );
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}
