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
  final OperationTracker _operations = OperationTracker();
  bool _markingRead = false;
  MessageDto? _replyingTo;

  @override
  void initState() {
    super.initState();
    _operations.addListener(_operationChanged);
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
    _operations.removeListener(_operationChanged);
    _operations.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _operationChanged() {
    if (mounted) setState(() {});
  }

  void _snapshotChanged() => unawaited(_markReadIfNeeded());

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
          final messages = snapshot.messages
              .where((message) => message.conversationId == widget.conversation.id)
              .toList(growable: false);
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
                              .where((attachment) => attachment.messageId == message.id)
                              .toList(growable: false);
                          final retryable =
                              message.direction == 'outbound' && message.status == 'failed';
                          final retryBusy = _operations.isActive('message:${message.id}:retry');
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
                                    RetryAttachmentCommandDto(attachmentIdHex: attachment.id),
                                  ),
                                  onCancel: () => _attachmentCommand(
                                    attachment.id,
                                    'cancel',
                                    CancelAttachmentCommandDto(attachmentIdHex: attachment.id),
                                  ),
                                  onOpen: () => _openAttachment(attachment),
                                  onSave: () => _saveAttachment(attachment),
                                );
                              }),
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
                            onPressed: pickingAttachment ? null : _pickAttachment,
                            icon: pickingAttachment
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
                              enabled: !sending,
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
        setState(() => _replyingTo = null);
      } else {
        _showError(BridgeErrorPresenter.message(result, fallback: 'Could not queue message'));
      }
    });
  }

  Future<void> _pickAttachment() async {
    await _operations.run('attachment:pick', () async {
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
          conversationIdHex: widget.conversation.id,
          sourcePath: path,
          name: file.name,
          mediaType: _mediaType(file.extension),
          size: file.size,
        ),
      );
      if (mounted && !response.ok) {
        _showError(BridgeErrorPresenter.message(response, fallback: 'Could not queue attachment'));
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
