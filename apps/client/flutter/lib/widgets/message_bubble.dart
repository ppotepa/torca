import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

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
    this.showBody = true,
    this.compactTop = false,
    this.senderLabel,
    super.key,
  });

  final MessageDto message;
  final VoidCallback onLongPress;
  final GestureTapDownCallback? onSecondaryTapDown;
  final String? quotedBody;
  final bool quotedUnavailable;
  final List<Widget> footer;
  final bool showBody;
  final bool compactTop;
  final String? senderLabel;

  @override
  Widget build(BuildContext context) {
    final outbound = message.direction == 'outbound';
    final background = outbound
        ? context.semanticColors.messageOutbound
        : context.semanticColors.messageInbound;
    final alignment = outbound ? Alignment.centerRight : Alignment.centerLeft;
    final normalRadius = context.torcaTokens.radiusLarge;
    final tailRadius = context.torcaTokens.radiusSmall;
    final radius = BorderRadius.only(
      topLeft: Radius.circular(
        compactTop && !outbound ? tailRadius : normalRadius,
      ),
      topRight: Radius.circular(
        compactTop && outbound ? tailRadius : normalRadius,
      ),
      bottomLeft: Radius.circular(outbound ? normalRadius : tailRadius),
      bottomRight: Radius.circular(outbound ? tailRadius : normalRadius),
    );

    return Align(
      alignment: alignment,
      child: LayoutBuilder(
        builder: (context, constraints) => ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: constraints.maxWidth < 560
                ? constraints.maxWidth * 0.84
                : 520,
          ),
          child: Padding(
            padding: EdgeInsets.fromLTRB(
              outbound ? 48 : 12,
              compactTop ? 1 : 4,
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
                    decoration: BoxDecoration(
                      color: background,
                      borderRadius: radius,
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        Row(
                          children: <Widget>[
                            Expanded(
                              child: Text(
                                senderLabel ?? (outbound ? 'You' : 'Contact'),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: Theme.of(context).textTheme.labelMedium
                                    ?.copyWith(fontWeight: FontWeight.w700),
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 6),
                        if (quotedBody != null) ...<Widget>[
                          ReplyQuote(
                            body: quotedBody!,
                            unavailable: quotedUnavailable,
                          ),
                          const SizedBox(height: 7),
                        ],
                        if (showBody) SelectableText(message.body),
                        if (footer.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 6),
                          ...footer,
                        ],
                        const SizedBox(height: 5),
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          mainAxisAlignment: MainAxisAlignment.end,
                          children: <Widget>[
                            if (outbound && message.sentAtMs != null)
                              _LifecycleMilestone(
                                kind: _LifecycleKind.sent,
                                milliseconds: message.sentAtMs!,
                              ),
                            if (outbound &&
                                message.deliveredAtMs != null) ...<Widget>[
                              const SizedBox(width: 5),
                              _LifecycleMilestone(
                                kind: _LifecycleKind.delivered,
                                milliseconds: message.deliveredAtMs!,
                              ),
                            ],
                            if (outbound &&
                                message.readAtMs != null) ...<Widget>[
                              const SizedBox(width: 5),
                              _LifecycleMilestone(
                                kind: _LifecycleKind.read,
                                milliseconds: message.readAtMs!,
                              ),
                            ],
                            if (!outbound || message.sentAtMs == null)
                              MessageTimestamp(
                                milliseconds: message.createdAtMs,
                              ),
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

enum _LifecycleKind { sent, delivered, read }

class _LifecycleMilestone extends StatelessWidget {
  const _LifecycleMilestone({required this.kind, required this.milliseconds});

  final _LifecycleKind kind;
  final int milliseconds;

  @override
  Widget build(BuildContext context) {
    final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    final value =
        '${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}';
    final (description, icon) = switch (kind) {
      _LifecycleKind.sent => ('Sent', context.torcaIcons.sent),
      _LifecycleKind.delivered => ('Delivered', context.torcaIcons.delivered),
      _LifecycleKind.read => ('Read', context.torcaIcons.read),
    };
    return Tooltip(
      message: '$description $value',
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Icon(icon, size: 12),
          const SizedBox(width: 2),
          Text(value, style: Theme.of(context).textTheme.labelSmall),
        ],
      ),
    );
  }
}
