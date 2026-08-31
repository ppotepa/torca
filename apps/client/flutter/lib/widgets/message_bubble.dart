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
      outbound: outbound,
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
        final maxBubbleWidth = usableWidth < 620 ? usableWidth * 0.80 : 540.0;
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
                  child: Material(
                    color: Colors.transparent,
                    clipBehavior: Clip.antiAlias,
                    borderRadius: radius,
                    child: InkWell(
                      borderRadius: radius,
                      onLongPress: onLongPress,
                      child: Ink(
                        key: ValueKey<String>('message-bubble-${message.id}'),
                        decoration: BoxDecoration(borderRadius: radius),
                        child: DefaultTextStyle.merge(
                          style: TextStyle(color: palette.foreground),
                          child: Column(
                            key: ValueKey<String>(
                              'message-content-${message.id}',
                            ),
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            mainAxisSize: MainAxisSize.min,
                            children: <Widget>[
                              if (compactTop)
                                Container(
                                  key: ValueKey<String>(
                                    'message-connector-${message.id}',
                                  ),
                                  height: 2,
                                  margin: const EdgeInsets.symmetric(
                                    horizontal: 6,
                                  ),
                                  color: palette.connector,
                                ),
                              if (showSender)
                                Container(
                                  key: ValueKey<String>(
                                    'message-header-${message.id}',
                                  ),
                                  height: 24,
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: 10,
                                  ),
                                  decoration: BoxDecoration(
                                    color: palette.header,
                                    borderRadius: BorderRadius.only(
                                      topLeft: radius.topLeft,
                                      topRight: radius.topRight,
                                    ),
                                  ),
                                  alignment: outbound
                                      ? Alignment.centerRight
                                      : Alignment.centerLeft,
                                  child: Text(
                                    sender,
                                    textAlign: outbound
                                        ? TextAlign.right
                                        : TextAlign.left,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: Theme.of(context)
                                        .textTheme
                                        .labelSmall
                                        ?.copyWith(
                                          fontWeight: FontWeight.w800,
                                          color: palette.headerForeground,
                                          height: 1,
                                        ),
                                  ),
                                ),
                              Container(
                                key: ValueKey<String>(
                                  'message-body-section-${message.id}',
                                ),
                                constraints: const BoxConstraints(
                                  minHeight: 52,
                                ),
                                padding: const EdgeInsets.fromLTRB(
                                  12,
                                  12,
                                  12,
                                  10,
                                ),
                                decoration: BoxDecoration(color: palette.body),
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
                                    if (reactionCounts.isNotEmpty) ...<Widget>[
                                      const SizedBox(height: 9),
                                      Wrap(
                                        spacing: 5,
                                        runSpacing: 4,
                                        children: <Widget>[
                                          for (final entry
                                              in reactionCounts.entries)
                                            Container(
                                              padding:
                                                  const EdgeInsets.symmetric(
                                                    horizontal: 7,
                                                    vertical: 3,
                                                  ),
                                              decoration: BoxDecoration(
                                                color: palette.footer,
                                                borderRadius:
                                                    BorderRadius.circular(
                                                      context
                                                              .torcaTokens
                                                              .terminal
                                                          ? 0
                                                          : 10,
                                                    ),
                                              ),
                                              child: Text(
                                                entry.value > 1
                                                    ? '${entry.key} ${entry.value}'
                                                    : entry.key,
                                                style: Theme.of(context)
                                                    .textTheme
                                                    .labelSmall
                                                    ?.copyWith(
                                                      color: palette.muted,
                                                      height: 1,
                                                    ),
                                              ),
                                            ),
                                        ],
                                      ),
                                    ],
                                    if (footer.isNotEmpty) ...<Widget>[
                                      const SizedBox(height: 9),
                                      ...footer,
                                    ],
                                  ],
                                ),
                              ),
                              Container(
                                key: ValueKey<String>(
                                  'message-footer-section-${message.id}',
                                ),
                                height: 24,
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 4,
                                ),
                                decoration: BoxDecoration(
                                  color: palette.footer,
                                  borderRadius: BorderRadius.only(
                                    bottomLeft: radius.bottomLeft,
                                    bottomRight: radius.bottomRight,
                                  ),
                                ),
                                child: Row(
                                  key: ValueKey<String>(
                                    'message-footer-${message.id}',
                                  ),
                                  children: <Widget>[
                                    if (outbound)
                                      _MessageActionButton(
                                        messageId: message.id,
                                        color: palette.muted,
                                        onPressed: onLongPress,
                                      ),
                                    if (outbound) const Spacer(),
                                    if (outbound && message.sentAtMs != null)
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
                                    if (!outbound) ...<Widget>[
                                      const Spacer(),
                                      _MessageActionButton(
                                        messageId: message.id,
                                        color: palette.muted,
                                        onPressed: onLongPress,
                                      ),
                                    ],
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
          ),
        );
      },
    );
  }
}

class _MessageActionButton extends StatelessWidget {
  const _MessageActionButton({
    required this.messageId,
    required this.color,
    required this.onPressed,
  });

  final String messageId;
  final Color color;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) => IconButton(
    key: ValueKey<String>('message-actions-$messageId'),
    tooltip: context.strings.messageActions,
    onPressed: onPressed,
    color: color,
    icon: Icon(context.torcaIcons.more, size: 14),
    style: IconButton.styleFrom(
      fixedSize: const Size.square(22),
      minimumSize: const Size.square(22),
      maximumSize: const Size.square(22),
      padding: EdgeInsets.zero,
      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
    ),
  );
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
