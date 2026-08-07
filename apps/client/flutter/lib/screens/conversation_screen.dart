import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

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
                      return ListTile(
                        title: Text(message.body),
                        subtitle: Text(message.status),
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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(result.error ?? 'Could not queue message')),
      );
    }
  }

  String _newId() {
    final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
    if (bytes.every((value) => value == 0)) bytes[15] = 1;
    return bytes.map((value) => value.toRadixString(16).padLeft(2, '0')).join();
  }
}
