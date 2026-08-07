import 'dart:async';
import 'dart:math';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

const int _maxAttachmentBytes = 16 * 1024 * 1024;

class ConversationScreen extends StatelessWidget {
  const ConversationScreen({required this.gateway, required this.conversation, super.key});
  final EngineGateway gateway;
  final ConversationDto conversation;

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('Conversation')),
    body: ConversationPane(gateway: gateway, conversation: conversation),
  );
}

class ConversationPane extends StatefulWidget {
  const ConversationPane({required this.gateway, required this.conversation, super.key});
  final EngineGateway gateway;
  final ConversationDto conversation;
  @override
  State<ConversationPane> createState() => _ConversationPaneState();
}

class _ConversationPaneState extends State<ConversationPane> {
  final TextEditingController _controller = TextEditingController();
  final Random _random = Random.secure();
  bool _markingRead = false;
  bool _pickingAttachment = false;

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
      unawaited(_markReadIfNeeded());
    }
  }

  @override
  void dispose() {
    widget.gateway.snapshots.removeListener(_snapshotChanged);
    _controller.dispose();
    super.dispose();
  }

  void _snapshotChanged() { unawaited(_markReadIfNeeded()); }

  Future<void> _markReadIfNeeded() async {
    if (_markingRead) return;
    final hasDeliveredInbound = widget.gateway.snapshots.value.messages.any(
      (message) => message.conversationId == widget.conversation.id &&
          message.direction == 'inbound' && message.status == 'delivered',
    );
    if (!hasDeliveredInbound) return;
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
      return Column(
        children: <Widget>[
          Expanded(
            child: messages.isEmpty
                ? const Center(child: Text('No messages yet'))
                : ListView.builder(
                    padding: const EdgeInsets.symmetric(vertical: 8),
                    itemCount: messages.length,
                    itemBuilder: (context, index) {
                      final message = messages[index];
                      final attachments = snapshot.attachments
                          .where((attachment) => attachment.messageId == message.id)
                          .toList(growable: false);
                      return ListTile(
                        title: Text(message.body),
                        subtitle: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            Text(message.status),
                            ...attachments.map((attachment) => _AttachmentProgress(
                              attachment: attachment,
                              onRetry: () => _attachmentCommand(
                                RetryAttachmentCommandDto(attachmentIdHex: attachment.id),
                              ),
                              onCancel: () => _attachmentCommand(
                                CancelAttachmentCommandDto(attachmentIdHex: attachment.id),
                              ),
                            )),
                          ],
                        ),
                        trailing: Text(message.direction),
                      );
                    },
                  ),
          ),
          const Divider(height: 1),
          SafeArea(
            top: false,
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Row(children: <Widget>[
                IconButton(
                  tooltip: 'Attach file',
                  onPressed: _pickingAttachment ? null : _pickAttachment,
                  icon: _pickingAttachment
                      ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                      : const Icon(Icons.attach_file),
                ),
                const SizedBox(width: 4),
                Expanded(
                  child: TextField(
                    controller: _controller,
                    minLines: 1,
                    maxLines: 5,
                    textInputAction: TextInputAction.newline,
                    decoration: const InputDecoration(labelText: 'Message', border: OutlineInputBorder()),
                  ),
                ),
                const SizedBox(width: 8),
                IconButton.filled(tooltip: 'Send message', icon: const Icon(Icons.send), onPressed: _sendMessage),
              ]),
            ),
          ),
        ],
      );
    },
  );

  Future<void> _sendMessage() async {
    final body = _controller.text.trim();
    if (body.isEmpty) return;
    final result = await widget.gateway.execute(
      QueueMessageCommandDto(
        messageIdHex: _newId(),
        conversationIdHex: widget.conversation.id,
        body: body,
        atMs: DateTime.now().millisecondsSinceEpoch,
      ),
    );
    if (!mounted) return;
    if (result.ok) {
      _controller.clear();
    } else {
      _showError(result.error ?? 'Could not queue message');
    }
  }

  Future<void> _pickAttachment() async {
    setState(() => _pickingAttachment = true);
    try {
      final result = await FilePicker.platform.pickFiles(
        allowMultiple: false,
        withData: false,
      );
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
      if (!mounted) return;
      if (!response.ok) _showError(response.error ?? 'Could not queue attachment');
    } finally {
      if (mounted) setState(() => _pickingAttachment = false);
    }
  }

  Future<void> _attachmentCommand(BridgeCommandDto command) async {
    final result = await widget.gateway.execute(command);
    if (mounted && !result.ok) _showError(result.error ?? 'Attachment operation failed');
  }

  void _showError(String text) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
  }

  String _mediaType(String? extension) {
    switch ((extension ?? '').toLowerCase()) {
      case 'jpg':
      case 'jpeg': return 'image/jpeg';
      case 'png': return 'image/png';
      case 'gif': return 'image/gif';
      case 'webp': return 'image/webp';
      case 'pdf': return 'application/pdf';
      case 'txt': return 'text/plain';
      case 'mp4': return 'video/mp4';
      case 'mp3': return 'audio/mpeg';
      default: return 'application/octet-stream';
    }
  }

  String _newId() {
    final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
    if (bytes.every((value) => value == 0)) bytes[15] = 1;
    return bytes.map((value) => value.toRadixString(16).padLeft(2, '0')).join();
  }
}

class _AttachmentProgress extends StatelessWidget {
  const _AttachmentProgress({required this.attachment, required this.onRetry, required this.onCancel});
  final AttachmentDto attachment;
  final VoidCallback onRetry;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final total = attachment.size <= 0 ? 1 : attachment.size;
    final progress = (attachment.offset / total).clamp(0.0, 1.0);
    final failed = attachment.status == 'failed';
    final terminal = attachment.status == 'available' || attachment.status == 'cancelled';
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: <Widget>[
        Row(children: <Widget>[
          const Icon(Icons.insert_drive_file_outlined, size: 18),
          const SizedBox(width: 6),
          Expanded(child: Text(attachment.name, overflow: TextOverflow.ellipsis)),
          Text(attachment.status),
        ]),
        const SizedBox(height: 4),
        LinearProgressIndicator(value: terminal && attachment.status == 'available' ? 1 : progress),
        if (failed || !terminal)
          Wrap(spacing: 8, children: <Widget>[
            if (failed) TextButton.icon(onPressed: onRetry, icon: const Icon(Icons.refresh), label: const Text('Retry')),
            if (!terminal) TextButton.icon(onPressed: onCancel, icon: const Icon(Icons.close), label: const Text('Cancel')),
          ]),
      ]),
    );
  }
}
