import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

class ConversationScreen extends StatelessWidget {
  const ConversationScreen({
    required this.gateway,
    required this.conversation,
    super.key,
  });

  final EngineGateway gateway;
  final ConversationDto conversation;

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Conversation')),
        body: ConversationPane(
          gateway: gateway,
          conversation: conversation,
        ),
      );
}

/// Shared conversation UI used by both compact navigation and wide split-view layouts.
class ConversationPane extends StatefulWidget {
  const ConversationPane({
    required this.gateway,
    required this.conversation,
    super.key,
  });

  final EngineGateway gateway;
  final ConversationDto conversation;

  @override
  State<ConversationPane> createState() => _ConversationPaneState();
}

class _ConversationPaneState extends State<ConversationPane> {
  final TextEditingController _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<AppSnapshotDto>(
      valueListenable: widget.gateway.snapshots,
      builder: (BuildContext context, AppSnapshotDto snapshot, Widget? child) {
        final List<MessageDto> messages = snapshot.messages
            .where(
              (MessageDto message) =>
                  message.conversationId == widget.conversation.id,
            )
            .toList(growable: false);

        return Column(
          children: <Widget>[
            Expanded(
              child: messages.isEmpty
                  ? const Center(child: Text('No messages yet'))
                  : ListView.builder(
                      padding: const EdgeInsets.symmetric(vertical: 8),
                      itemCount: messages.length,
                      itemBuilder: (BuildContext context, int index) {
                        final MessageDto message = messages[index];
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
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: TextField(
                        controller: _controller,
                        minLines: 1,
                        maxLines: 5,
                        textInputAction: TextInputAction.newline,
                        decoration: const InputDecoration(
                          labelText: 'Message',
                          border: OutlineInputBorder(),
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
              ),
            ),
          ],
        );
      },
    );
  }

  Future<void> _sendMessage() async {
    final String body = _controller.text.trim();
    if (body.isEmpty) {
      return;
    }

    final String messageId = DateTime.now()
        .microsecondsSinceEpoch
        .toRadixString(16)
        .padLeft(32, '0');
    final BridgeResultDto result = await widget.gateway.execute(
      QueueMessageCommandDto(
        messageIdHex: messageId,
        conversationIdHex: widget.conversation.id,
        body: body,
        atMs: DateTime.now().millisecondsSinceEpoch,
      ),
    );

    if (!mounted) {
      return;
    }
    if (result.ok) {
      _controller.clear();
      return;
    }

    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(result.error ?? 'Could not queue message')),
    );
  }
}
