import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';
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
    this.reactions = const <ReactionDto>[],
    this.showBody = true,
    this.compactTop = false,
    this.compactBottom = false,
    this.showSender = true,
    this.senderLabel,
    this.senderColorKey,
    super.key,
  });

  final MessageDto message;
  final VoidCallback onLongPress;
  final GestureTapDownCallback? onSecondaryTapDown;
  final String? quotedBody;
  final bool quotedUnavailable;
  final List<Widget> footer;
  final List<ReactionDto> reactions;
  final bool showBody;
  final bool compactTop;
  final bool compactBottom;
  final bool showSender;
  final String? senderLabel;
  final String? senderColorKey;

  @override
  Widget build(BuildContext context) {
    final outbound = message.typedDirection == MessageDirection.outbound;
    final reactionCounts = <String, int>{};
    for (final reaction in reactions) {
      if (!reaction.active) continue;
      reactionCounts.update(
        reaction.emoji,
        (count) => count + 1,
        ifAbsent: () => 1,
      );
    }
    final palette = TorcaMessagePalette.resolve(
      context,
      senderColorKey ?? (outbound ? 'local' : 'remote'),
    );
    final sender =
        senderLabel ??
        (outbound ? context.strings.senderYou : context.strings.senderContact);
    final semanticBody = showBody
        ? message.body
        : message.typedStatus == MessageStatus.deleted
        ? context.strings.messageDeleted
        : context.strings.attachmentMessagePending;
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
      bottomLeft: Radius.circular(
        compactBottom ? tailRadius : (outbound ? normalRadius : tailRadius),
      ),
      bottomRight: Radius.circular(
        compactBottom ? tailRadius : (outbound ? tailRadius : normalRadius),
      ),
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        const horizontalGutter = 12.0;
        final usableWidth = constraints.maxWidth - horizontalGutter * 2;
        final maxBubbleWidth = usableWidth < 620 ? usableWidth * 0.84 : 520.0;
        return Padding(
          padding: EdgeInsets.fromLTRB(
            horizontalGutter,
            compactTop ? 1 : 4,
            horizontalGutter,
            compactBottom ? 1 : 4,
          ),
          child: Align(
            alignment: alignment,
            child: ConstrainedBox(
              constraints: BoxConstraints(maxWidth: maxBubbleWidth),
              child: Semantics(
                container: true,
                label: '$sender: $semanticBody',
                child: GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onSecondaryTapDown: onSecondaryTapDown,
                  child: InkWell(
                    borderRadius: radius,
                    onLongPress: onLongPress,
                    child: Ink(
                      key: ValueKey<String>('message-bubble-${message.id}'),
                      padding: const EdgeInsets.fromLTRB(12, 10, 10, 8),
                      decoration: BoxDecoration(
                        color: palette.surface,
                        border: Border.all(
                          color: palette.border,
                          width: context.torcaTokens.terminal ? 1.4 : 1.2,
                        ),
                        borderRadius: radius,
                      ),
                      child: DefaultTextStyle.merge(
                        style: TextStyle(color: palette.foreground),
                        child: Column(
                          key: ValueKey<String>(
                            'message-content-${message.id}',
                          ),
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            Container(
                              key: ValueKey<String>(
                                'message-header-${message.id}',
                              ),
                              padding: const EdgeInsets.only(bottom: 6),
                              decoration: BoxDecoration(
                                border: Border(
                                  bottom: BorderSide(
                                    color: palette.border.withValues(
                                      alpha: .48,
                                    ),
                                  ),
                                ),
                              ),
                              child: Row(
                                children: <Widget>[
                                  Expanded(
                                    child: showSender
                                        ? Text(
                                            sender,
                                            maxLines: 1,
                                            overflow: TextOverflow.ellipsis,
                                            style: Theme.of(context)
                                                .textTheme
                                                .labelMedium
                                                ?.copyWith(
                                                  fontWeight: FontWeight.w800,
                                                  color: palette.foreground,
                                                ),
                                          )
                                        : const SizedBox.shrink(),
                                  ),
                                  IconButton(
                                    key: ValueKey<String>(
                                      'message-actions-${message.id}',
                                    ),
                                    tooltip: context.strings.messageActions,
                                    onPressed: onLongPress,
                                    icon: Icon(context.torcaIcons.more),
                                    visualDensity: VisualDensity.compact,
                                    constraints: const BoxConstraints(
                                      minWidth: 32,
                                      minHeight: 32,
                                    ),
                                    padding: EdgeInsets.zero,
                                  ),
                                ],
                              ),
                            ),
                            const SizedBox(height: 8),
                            Container(
                              key: ValueKey<String>(
                                'message-body-section-${message.id}',
                              ),
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                mainAxisSize: MainAxisSize.min,
                                children: <Widget>[
                                  if (quotedBody != null) ...<Widget>[
                                    ReplyQuote(
                                      body: quotedBody!,
                                      unavailable: quotedUnavailable,
                                    ),
                                    const SizedBox(height: 7),
                                  ],
                                  if (showBody)
                                    SelectableText(
                                      message.body,
                                      key: ValueKey<String>(
                                        'message-body-${message.id}',
                                      ),
                                      style: Theme.of(context)
                                          .textTheme
                                          .bodyMedium
                                          ?.copyWith(
                                            color: palette.foreground,
                                            height: 1.42,
                                          ),
                                    )
                                  else if (message.typedStatus ==
                                      MessageStatus.deleted)
                                    Text(
                                      context.strings.messageDeleted,
                                      key: ValueKey<String>(
                                        'message-body-${message.id}',
                                      ),
                                      style: Theme.of(context)
                                          .textTheme
                                          .bodyMedium
                                          ?.copyWith(
                                            color: palette.foreground,
                                            fontStyle: FontStyle.italic,
                                          ),
                                    ),
                                ],
                              ),
                            ),
                            Container(
                              key: ValueKey<String>(
                                'message-footer-section-${message.id}',
                              ),
                              margin: const EdgeInsets.only(top: 9),
                              padding: const EdgeInsets.only(top: 7),
                              decoration: BoxDecoration(
                                border: Border(
                                  top: BorderSide(
                                    color: palette.border.withValues(
                                      alpha: .52,
                                    ),
                                  ),
                                ),
                              ),
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.stretch,
                                children: <Widget>[
                                  if (reactionCounts.isNotEmpty)
                                    Wrap(
                                      spacing: 4,
                                      children: <Widget>[
                                        for (final entry
                                            in reactionCounts.entries)
                                          Chip(
                                            label: Text(
                                              entry.value > 1
                                                  ? '${entry.key} ${entry.value}'
                                                  : entry.key,
                                            ),
                                            visualDensity:
                                                VisualDensity.compact,
                                            materialTapTargetSize:
                                                MaterialTapTargetSize
                                                    .shrinkWrap,
                                          ),
                                      ],
                                    ),
                                  if (footer.isNotEmpty) ...<Widget>[
                                    if (reactionCounts.isNotEmpty)
                                      const SizedBox(height: 5),
                                    ...footer,
                                  ],
                                  Align(
                                    alignment: Alignment.centerRight,
                                    child: Row(
                                      key: ValueKey<String>(
                                        'message-footer-${message.id}',
                                      ),
                                      mainAxisSize: MainAxisSize.min,
                                      children: <Widget>[
                                        if (outbound &&
                                            message.sentAtMs != null)
                                          _LifecycleTimeline(message: message)
                                        else ...<Widget>[
                                          MessageTimestamp(
                                            milliseconds: message.createdAtMs,
                                          ),
                                          if (outbound) ...<Widget>[
                                            const SizedBox(width: 5),
                                            MessageStatusIndicator(
                                              status: message.typedStatus,
                                            ),
                                          ],
                                        ],
                                      ],
                                    ),
                                  ),
                                ],
                              ),
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
      },
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
      _LifecycleKind.sent => (context.strings.sent, context.torcaIcons.sent),
      _LifecycleKind.delivered => (
        context.strings.delivered,
        context.torcaIcons.delivered,
      ),
      _LifecycleKind.read => (context.strings.read, context.torcaIcons.read),
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

class _LifecycleTimeline extends StatelessWidget {
  const _LifecycleTimeline({required this.message});

  final MessageDto message;

  @override
  Widget build(BuildContext context) {
    final (kind, timestamp) = message.readAtMs != null
        ? (_LifecycleKind.read, message.readAtMs!)
        : message.deliveredAtMs != null
        ? (_LifecycleKind.delivered, message.deliveredAtMs!)
        : (_LifecycleKind.sent, message.sentAtMs!);
    return _LifecycleMilestone(kind: kind, milliseconds: timestamp);
  }
}
