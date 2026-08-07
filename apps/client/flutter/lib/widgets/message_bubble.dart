import 'package:flutter/material.dart';

import '../generated/torca_contract.dart';
import '../theme/app_semantic_colors.dart';
import 'message_status_indicator.dart';
import 'message_timestamp.dart';
import 'reply_quote.dart';

class MessageBubble extends StatelessWidget {
  const MessageBubble({
    required this.message,
    required this.onLongPress,
    this.onSecondaryTapDown,
    this.quotedBody,
    this.quotedUnavailable = false,
    this.footer = const <Widget>[],
    super.key,
  });

  final MessageDto message;
  final VoidCallback onLongPress;
  final GestureTapDownCallback? onSecondaryTapDown;
  final String? quotedBody;
  final bool quotedUnavailable;
  final List<Widget> footer;

  @override
  Widget build(BuildContext context) {
    final outbound = message.direction == 'outbound';
    final background = outbound
        ? context.semanticColors.messageOutbound
        : context.semanticColors.messageInbound;
    final alignment = outbound ? Alignment.centerRight : Alignment.centerLeft;
    final radius = BorderRadius.only(
      topLeft: const Radius.circular(16),
      topRight: const Radius.circular(16),
      bottomLeft: Radius.circular(outbound ? 16 : 4),
      bottomRight: Radius.circular(outbound ? 4 : 16),
    );

    return Align(
      alignment: alignment,
      child: LayoutBuilder(
        builder: (context, constraints) => ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: constraints.maxWidth < 560 ? constraints.maxWidth * 0.84 : 520,
          ),
          child: Padding(
            padding: EdgeInsets.fromLTRB(
              outbound ? 48 : 12,
              4,
              outbound ? 12 : 48,
              4,
            ),
            child: Semantics(
              label: outbound ? 'Outgoing message' : 'Incoming message',
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onSecondaryTapDown: onSecondaryTapDown,
                child: InkWell(
                  borderRadius: radius,
                  onLongPress: onLongPress,
                  child: Ink(
                    padding: const EdgeInsets.fromLTRB(12, 10, 10, 7),
                    decoration: BoxDecoration(color: background, borderRadius: radius),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        if (quotedBody != null) ...<Widget>[
                          ReplyQuote(body: quotedBody!, unavailable: quotedUnavailable),
                          const SizedBox(height: 7),
                        ],
                        SelectableText(message.body),
                        if (footer.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 6),
                          ...footer,
                        ],
                        const SizedBox(height: 5),
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          mainAxisAlignment: MainAxisAlignment.end,
                          children: <Widget>[
                            MessageTimestamp(milliseconds: message.createdAtMs),
                            if (outbound) ...<Widget>[
                              const SizedBox(width: 5),
                              MessageStatusIndicator(status: message.status),
                            ],
                          ],
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
