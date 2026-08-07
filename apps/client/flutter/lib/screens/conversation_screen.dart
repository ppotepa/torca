import 'dart:async';
import 'dart:io';
import 'dart:math';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:open_filex/open_filex.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../widgets/conversation_header.dart';
import '../widgets/message_actions.dart';
import '../widgets/message_bubble.dart';
import 'connection_details_screen.dart';

const int _maxAttachmentBytes = 16 * 1024 * 1024;

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
  final Random _random = Random.secure();
  bool _markingRead = false;
  bool _pickingAttachment = false;
  MessageDto? _replyingTo;

  @override
  void initState() {
    super.initState();
    widget.gateway.snapshots.addListener(_snapshotChanged);
    WidgetsBinding.instance.addPostFrameCallback((_) => unawaited(_markReadIfNeeded()));
  }

  @override
  void didUpdateWidget(covariant ConversationPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.gateway != widget.gateway) {
      oldWidget.gateway.snapshots.removeListener(_snapshotChanged);
      widget.gateway.snapshots.addListener(_snapshotChanged);
    }
    if (oldWidget.conversation.id != widget.conversation.id) {
      _replyingTo = null;
      unawaited(_markReadIfNeeded());
    }
  }

  @override
  void dispose() {
    widget.gateway.snapshots.removeListener(_snapshotChanged);
    _controller.dispose();
    super.dispose();
  }

  void _snapshotChanged() => unawaited(_markReadIfNeeded());

  Future<void> _markReadIfNeeded() async {
    if (_markingRead) return;
    final hasDelivered = widget.gateway.snapshots.value.messages.any(
      (m) =>
          m.conversationId == widget.conversation.id &&
          m.direction == 'inbound' &&
          m.status == 'delivered',
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
          final messages = snapshot.messages
              .where((m) => m.conversationId == widget.conversation.id)
              .toList(growable: false);
          final byId = <String, MessageDto>{for (final m in messages) m.id: m};
          final reply = _replyingTo;
          final contact = _contactFor(snapshot, widget.conversation);
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
                child: messages.isEmpty
                    ? const Center(
                        child: Padding(
                          padding: EdgeInsets.all(24),
                          child: Text(
                            'No messages yet. Messages are sent directly through Tor.',
                            textAlign: TextAlign.center,
                          ),
                        ),
                      )
                    : ListView.builder(
                        padding: const EdgeInsets.symmetric(vertical: 8),
                        itemCount: messages.length,
                        itemBuilder: (context, index) {
                          final message = messages[index];
                          final quoted = message.replyToMessageId == null
                              ? null
                              : byId[message.replyToMessageId];
                          final attachments = snapshot.attachments
                              .where((a) => a.messageId == message.id)
                              .toList(growable: false);
                          final retryable =
                              message.direction == 'outbound' && message.status == 'failed';
                          return MessageBubble(
                            message: message,
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
                                    onPressed: () => _retryMessage(message),
                                    icon: const Icon(Icons.refresh),
                                    label: const Text('Retry now'),
                                  ),
                                ),
                              ...attachments.map(
                                (a) => _AttachmentProgress(
                                  attachment: a,
                                  onRetry: () => _attachmentCommand(
                                    RetryAttachmentCommandDto(attachmentIdHex: a.id),
                                  ),
                                  onCancel: () => _attachmentCommand(
                                    CancelAttachmentCommandDto(attachmentIdHex: a.id),
                                  ),
                                  onOpen: () => _openAttachment(a),
                                  onSave: () => _saveAttachment(a),
                                ),
                              ),
                            ],
                          );
                        },
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
                            tooltip: 'Attach file',
                            onPressed: _pickingAttachment ? null : _pickAttachment,
                            icon: _pickingAttachment
                                ? const SizedBox(
                                    width: 20,
                                    height: 20,
                                    child: CircularProgressIndicator(strokeWidth: 2),
                                  )
                                : const Icon(Icons.attach_file),
                          ),
                          const SizedBox(width: 4),
                          Expanded(
                            child: TextField(
                              controller: _controller,
                              minLines: 1,
                              maxLines: 5,
                              textInputAction: TextInputAction.newline,
                              decoration: InputDecoration(
                                labelText: reply == null ? 'Message' : 'Reply',
                              ),
                            ),
                          ),
                          const SizedBox(width: 8),
                          IconButton.filled(
                            tooltip: 'Send message',
                            icon: const Icon(Icons.send),
                            onPressed: _sendMessage,
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

  Future<void> _retryMessage(MessageDto m) async {
    final r = await widget.gateway.execute(
      RetryMessageCommandDto(
        messageIdHex: m.id,
        atMs: DateTime.now().millisecondsSinceEpoch,
      ),
    );
    if (mounted && !r.ok) _showError(r.error ?? 'Could not retry message');
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

  Future<void> _showMessageDetails(MessageDto m) => showDialog<void>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('Message details'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              _detail('ID', m.id),
              _detail('Direction', m.direction),
              _detail('Status', _messageStatusLabel(m.status)),
              _detail('Queued / received', _date(m.createdAtMs)),
              _detail('Last update', _date(m.updatedAtMs)),
              _detail('Send attempts', '${m.attemptCount}'),
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
    final r = await widget.gateway.execute(
      QueueMessageCommandDto(
        messageIdHex: _newId(),
        conversationIdHex: widget.conversation.id,
        body: body,
        replyToMessageId: _replyingTo?.id,
        atMs: DateTime.now().millisecondsSinceEpoch,
      ),
    );
    if (!mounted) return;
    if (r.ok) {
      _controller.clear();
      setState(() => _replyingTo = null);
    } else {
      _showError(r.error ?? 'Could not queue message');
    }
  }

  Future<void> _pickAttachment() async {
    setState(() => _pickingAttachment = true);
    try {
      final result = await FilePicker.platform.pickFiles(allowMultiple: false, withData: false);
      if (result == null || result.files.isEmpty || !mounted) return;
      final file = result.files.single;
      final path = file.path;
      if (path == null || path.isEmpty) {
        _showError('The selected file is not available as a local file');
        return;
      }
      if (file.size <= 0 || file.size > _maxAttachmentBytes) {
        _showError('Attachments must be between 1 byte and 16 MiB');
        return;
      }
      final response = await widget.gateway.execute(
        QueueAttachmentCommandDto(
          attachmentIdHex: _newId(),
          messageIdHex: _newId(),
          conversationIdHex: widget.conversation.id,
          sourcePath: path,
          name: file.name,
          mediaType: _mediaType(file.extension),
          size: file.size,
        ),
      );
      if (mounted && !response.ok) _showError(response.error ?? 'Could not queue attachment');
    } finally {
      if (mounted) setState(() => _pickingAttachment = false);
    }
  }

  Future<void> _saveAttachment(AttachmentDto a) async {
    final path = await FilePicker.platform.saveFile(dialogTitle: 'Save attachment', fileName: a.name);
    if (path == null || !mounted) return;
    final r = await widget.gateway.execute(
      ExportAttachmentCommandDto(attachmentIdHex: a.id, destinationPath: path),
    );
    if (mounted) {
      if (r.ok) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Attachment saved')),
        );
      } else {
        _showError(r.error ?? 'Could not save attachment');
      }
    }
  }

  Future<void> _openAttachment(AttachmentDto a) async {
    final ext = _safeExtension(a.name);
    final path = '${Directory.systemTemp.path}${Platform.pathSeparator}torca-${a.id}$ext';
    final r = await widget.gateway.execute(
      ExportAttachmentCommandDto(attachmentIdHex: a.id, destinationPath: path),
    );
    if (!mounted) return;
    if (!r.ok) {
      _showError(r.error ?? 'Could not open attachment');
      return;
    }
    final opened = await OpenFilex.open(path);
    if (mounted && opened.type != ResultType.done) _showError(opened.message);
  }

  String _safeExtension(String name) {
    final dot = name.lastIndexOf('.');
    if (dot < 0 || dot == name.length - 1) return '';
    final value = name.substring(dot);
    return RegExp(r'^\.[A-Za-z0-9]{1,10}$').hasMatch(value)
        ? value.toLowerCase()
        : '';
  }

  Future<void> _attachmentCommand(BridgeCommandDto c) async {
    final r = await widget.gateway.execute(c);
    if (mounted && !r.ok) _showError(r.error ?? 'Attachment operation failed');
  }

  void _showError(String text) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
  }

  String _mediaType(String? e) {
    switch ((e ?? '').toLowerCase()) {
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

  String _newId() {
    final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
    if (bytes.every((v) => v == 0)) bytes[15] = 1;
    return bytes.map((v) => v.toRadixString(16).padLeft(2, '0')).join();
  }

  String _messageStatusLabel(String s) => switch (s) {
        'queued' => 'Queued — waiting for a direct peer connection',
        'sending' => 'Sending…',
        'sent' => 'Sent',
        'delivered' => 'Delivered',
        'read' => 'Read',
        'failed' => 'Delivery failed',
        'cancelled' => 'Cancelled',
        _ => s,
      };
}

ContactDto? _contactFor(AppSnapshotDto snapshot, ConversationDto conversation) {
  for (final contact in snapshot.contacts) {
    if (contact.id == conversation.contactId) return contact;
  }
  return null;
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
            Expanded(child: Text(message.body, maxLines: 2, overflow: TextOverflow.ellipsis)),
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

class _AttachmentProgress extends StatelessWidget {
  const _AttachmentProgress({
    required this.attachment,
    required this.onRetry,
    required this.onCancel,
    required this.onOpen,
    required this.onSave,
  });
  final AttachmentDto attachment;
  final VoidCallback onRetry, onCancel, onOpen, onSave;

  @override
  Widget build(BuildContext context) {
    final total = attachment.size <= 0 ? 1 : attachment.size;
    final progress = (attachment.offset / total).clamp(0.0, 1.0);
    final failed = attachment.status == 'failed';
    final available = attachment.status == 'available';
    final terminal = available || attachment.status == 'cancelled';
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              const Icon(Icons.insert_drive_file_outlined, size: 18),
              const SizedBox(width: 6),
              Expanded(child: Text(attachment.name, overflow: TextOverflow.ellipsis)),
              Text(attachment.status),
            ],
          ),
          const SizedBox(height: 4),
          LinearProgressIndicator(value: available ? 1 : progress),
          Wrap(
            spacing: 8,
            children: <Widget>[
              if (available)
                TextButton.icon(
                  onPressed: onOpen,
                  icon: const Icon(Icons.open_in_new),
                  label: const Text('Open'),
                ),
              if (available)
                TextButton.icon(
                  onPressed: onSave,
                  icon: const Icon(Icons.save_alt),
                  label: const Text('Save as'),
                ),
              if (failed)
                TextButton.icon(
                  onPressed: onRetry,
                  icon: const Icon(Icons.refresh),
                  label: const Text('Retry'),
                ),
              if (!terminal)
                TextButton.icon(
                  onPressed: onCancel,
                  icon: const Icon(Icons.close),
                  label: const Text('Cancel'),
                ),
            ],
          ),
        ],
      ),
    );
  }
}
