import 'package:flutter/material.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

class ConversationScreen extends StatefulWidget {
  const ConversationScreen({
    required this.gateway,
    required this.conversation,
    super.key,
  });

  final EngineGateway gateway;
  final ConversationDto conversation;

  @override
  State<ConversationScreen> createState() => _ConversationScreenState();
}

class _ConversationScreenState extends State<ConversationScreen> {
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

        return Scaffold(
          appBar: AppBar(title: const Text('Conversation')),
          body: Column(
            children: <Widget>[
              Expanded(
                child: ListView.builder(
                  itemCount: messages.length,
                  itemBuilder: (BuildContext context, int index) {
                    final MessageDto message = messages[index];
                    return ListTile(
                      title: Text(message.body),
                      subtitle: Text(message.status),
                    );
                  },
                ),
              ),
              Padding(
                padding: const EdgeInsets.all(12),
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: TextField(
                        controller: _controller,
                        decoration: const InputDecoration(
                          labelText: 'Message',
                        ),
                      ),
                    ),
                    IconButton(
                      icon: const Icon(Icons.send),
                      onPressed: _sendMessage,
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Future<void> _sendMessage() async {
    final String body = _controller.text;
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
