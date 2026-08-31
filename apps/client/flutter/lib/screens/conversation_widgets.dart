part of 'conversation_screen.dart';

/// The single responsive frame used by mobile routes and desktop panes.
/// Header and composer never participate in scrolling; only [content] owns
/// the remaining viewport and its scroll controller.
class ConversationContainer extends StatelessWidget {
  const ConversationContainer({
    required this.content,
    required this.footer,
    this.header,
    super.key,
  });

  final Widget? header;
  final Widget content;
  final Widget footer;

  @override
  Widget build(BuildContext context) => ColoredBox(
    color: Theme.of(context).colorScheme.surface,
    child: ClipRect(
      child: Column(
        children: <Widget>[
          if (header != null) header!,
          Expanded(child: content),
          const Divider(height: 1),
          footer,
        ],
      ),
    ),
  );
}

/// Shared composer surface for both the mobile route and desktop pane.
/// Layout decisions stay here while ConversationPane owns persistence and
/// command orchestration.
class ConversationComposer extends StatelessWidget {
  const ConversationComposer({
    required this.gateway,
    required this.messageField,
    required this.contact,
    required this.radio,
    required this.session,
    required this.pendingAttachments,
    required this.onRemoveAttachment,
    required this.onPickAttachments,
    required this.onInsertEmoji,
    required this.onSend,
    required this.onVoiceClipReady,
    required this.sending,
    required this.sendingAttachment,
    required this.pickingAttachment,
    required this.searching,
    this.disabled = false,
    this.disabledMessage,
    this.reply,
    this.onCancelReply,
    super.key,
  });

  final EngineGateway gateway;
  final Widget messageField;
  final ContactDto? contact;
  final RadioContactDto? radio;
  final RadioSessionDto? session;
  final List<_PendingAttachment> pendingAttachments;
  final ValueChanged<_PendingAttachment> onRemoveAttachment;
  final VoidCallback onPickAttachments;
  final VoidCallback onInsertEmoji;
  final VoidCallback onSend;
  final VoiceClipReady onVoiceClipReady;
  final bool sending;
  final bool sendingAttachment;
  final bool pickingAttachment;
  final bool searching;
  final bool disabled;
  final String? disabledMessage;
  final MessageDto? reply;
  final VoidCallback? onCancelReply;

  @override
  Widget build(BuildContext context) => SafeArea(
    top: false,
    child: Padding(
      padding: const EdgeInsets.all(12),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (pendingAttachments.isNotEmpty) ...<Widget>[
            _AttachmentTray(
              attachments: pendingAttachments,
              onRemove: onRemoveAttachment,
            ),
            const SizedBox(height: 8),
          ],
          if (reply != null) ...<Widget>[
            _ReplyComposerPreview(
              message: reply!,
              onCancel: onCancelReply ?? () {},
            ),
            const SizedBox(height: 8),
          ],
          if (disabledMessage != null) ...<Widget>[
            Semantics(
              liveRegion: true,
              child: Container(
                width: double.infinity,
                padding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 9,
                ),
                decoration: BoxDecoration(
                  color: Theme.of(context).colorScheme.errorContainer,
                  borderRadius: BorderRadius.circular(
                    context.torcaTokens.radiusMedium,
                  ),
                ),
                child: Row(
                  children: <Widget>[
                    Icon(
                      context.torcaIcons.warning,
                      color: Theme.of(context).colorScheme.onErrorContainer,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        disabledMessage!,
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.onErrorContainer,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 8),
          ],
          LayoutBuilder(
            builder: (context, constraints) {
              final narrow = constraints.maxWidth < 430;
              final emoji = IconButton(
                tooltip: context.strings.emoji,
                onPressed: disabled || searching || sending || sendingAttachment
                    ? null
                    : onInsertEmoji,
                icon: Icon(context.torcaIcons.emoji),
              );
              final attachment = IconButton(
                tooltip: context.strings.attachFiles,
                onPressed:
                    disabled ||
                        pickingAttachment ||
                        sendingAttachment ||
                        searching
                    ? null
                    : onPickAttachments,
                icon: pickingAttachment
                    ? const SizedBox(
                        width: 20,
                        height: 20,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Icon(context.torcaIcons.attachment),
              );
              final send = IconButton.filled(
                tooltip: context.strings.sendMessage,
                onPressed: disabled || sending || sendingAttachment || searching
                    ? null
                    : onSend,
                icon: sending || sendingAttachment
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Icon(context.torcaIcons.send),
              );
              final voice = contact == null
                  ? null
                  : radio?.localEnabled == true
                  ? RadioPushToTalk(
                      gateway: gateway,
                      contact: contact!,
                      radio: radio!,
                      session: session,
                      disabled: disabled || searching,
                    )
                  : VoiceClipRecorder(
                      onClipReady: onVoiceClipReady,
                      disabled:
                          disabled || searching || sending || sendingAttachment,
                    );

              if (!narrow) {
                return Row(
                  children: <Widget>[
                    emoji,
                    attachment,
                    const SizedBox(width: 4),
                    Expanded(child: messageField),
                    const SizedBox(width: 8),
                    send,
                    if (voice != null) ...<Widget>[
                      const SizedBox(width: 4),
                      voice,
                    ],
                  ],
                );
              }

              // Keep the text field and its send controls together on phones.
              // The secondary actions get their own row so a 320px viewport
              // never compresses the editable area into an unusable sliver.
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Row(children: <Widget>[emoji, attachment, const Spacer()]),
                  const SizedBox(height: 4),
                  Row(
                    children: <Widget>[
                      Expanded(child: messageField),
                      const SizedBox(width: 8),
                      send,
                      if (voice != null) ...<Widget>[
                        const SizedBox(width: 4),
                        voice,
                      ],
                    ],
                  ),
                ],
              );
            },
          ),
        ],
      ),
    ),
  );
}

class _DateSeparator extends StatelessWidget {
  const _DateSeparator({required this.milliseconds});
  final int milliseconds;

  @override
  Widget build(BuildContext context) {
    final date = DateTime.fromMillisecondsSinceEpoch(milliseconds).toLocal();
    final now = DateTime.now();
    final today = DateTime(now.year, now.month, now.day);
    final day = DateTime(date.year, date.month, date.day);
    final difference = today.difference(day).inDays;
    final label = switch (difference) {
      0 => context.strings.today,
      1 => context.strings.yesterday,
      _ =>
        '${date.year}-${date.month.toString().padLeft(2, '0')}-${date.day.toString().padLeft(2, '0')}',
    };
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Center(
        child: Text(label, style: Theme.of(context).textTheme.labelSmall),
      ),
    );
  }
}

class _UnreadSeparator extends StatelessWidget {
  const _UnreadSeparator();

  @override
  Widget build(BuildContext context) => Row(
    children: <Widget>[
      const Expanded(child: Divider()),
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        child: Text(context.strings.newMessages),
      ),
      const Expanded(child: Divider()),
    ],
  );
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
      borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
    ),
    child: Row(
      children: <Widget>[
        Icon(context.torcaIcons.reply, size: 18),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            message.body,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
          ),
        ),
        IconButton(
          tooltip: context.strings.cancel,
          visualDensity: VisualDensity.compact,
          onPressed: onCancel,
          icon: Icon(context.torcaIcons.close),
        ),
      ],
    ),
  );
}

class _PendingAttachment {
  const _PendingAttachment(this.originalName, this.prepared);

  final String originalName;
  final PreparedAttachment prepared;
}

class _AttachmentTray extends StatelessWidget {
  const _AttachmentTray({required this.attachments, required this.onRemove});

  final List<_PendingAttachment> attachments;
  final ValueChanged<_PendingAttachment> onRemove;

  @override
  Widget build(BuildContext context) => Container(
    constraints: const BoxConstraints(minHeight: 72, maxHeight: 116),
    width: double.infinity,
    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      borderRadius: BorderRadius.circular(context.torcaTokens.radiusLarge),
    ),
    child: ListView.separated(
      scrollDirection: Axis.horizontal,
      itemCount: attachments.length,
      separatorBuilder: (_, _) => const SizedBox(width: 8),
      itemBuilder: (context, index) {
        final item = attachments[index];
        final prepared = item.prepared;
        // A video processor can provide a lightweight JPEG cover via
        // `previewPath`; the composer should render it exactly like an image
        // preview instead of regressing to a generic file icon.  This keeps
        // the tray independent from the platform-specific frame extractor.
        final previewPath = prepared.kind == AttachmentMediaKind.image
            ? prepared.path
            : prepared.previewPath;
        return SizedBox(
          width: 190,
          child: Row(
            children: <Widget>[
              ClipRRect(
                borderRadius: BorderRadius.circular(
                  context.torcaTokens.radiusSmall,
                ),
                child: SizedBox(
                  width: 52,
                  height: 52,
                  child: previewPath != null
                      ? Image.file(File(previewPath), fit: BoxFit.cover)
                      : ColoredBox(
                          color: Theme.of(context).colorScheme.surface,
                          child: Icon(_iconFor(context, prepared.kind)),
                        ),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      item.originalName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.labelMedium,
                    ),
                    Text(
                      '${_kindLabel(prepared.kind)} / ${formatBytes(prepared.size)}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.labelSmall,
                    ),
                  ],
                ),
              ),
              IconButton(
                tooltip: context.strings.removeAttachment,
                visualDensity: VisualDensity.compact,
                onPressed: () => onRemove(item),
                icon: Icon(context.torcaIcons.close),
              ),
            ],
          ),
        );
      },
    ),
  );

  static IconData _iconFor(BuildContext context, AttachmentMediaKind kind) =>
      switch (kind) {
        AttachmentMediaKind.video => context.torcaIcons.video,
        AttachmentMediaKind.audio => context.torcaIcons.audio,
        AttachmentMediaKind.pdf => context.torcaIcons.pdf,
        AttachmentMediaKind.document => context.torcaIcons.document,
        AttachmentMediaKind.archive => context.torcaIcons.archive,
        AttachmentMediaKind.text => context.torcaIcons.textFile,
        AttachmentMediaKind.image => context.torcaIcons.image,
        AttachmentMediaKind.binary => context.torcaIcons.file,
      };

  static String _kindLabel(AttachmentMediaKind kind) => switch (kind) {
    AttachmentMediaKind.image => 'Image',
    AttachmentMediaKind.video => 'Video',
    AttachmentMediaKind.audio => 'Audio',
    AttachmentMediaKind.pdf => 'PDF document',
    AttachmentMediaKind.document => 'Document',
    AttachmentMediaKind.archive => 'Archive',
    AttachmentMediaKind.text => 'Text file',
    AttachmentMediaKind.binary => 'File',
  };
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}

class _ConversationSearchBar extends StatelessWidget {
  const _ConversationSearchBar({
    required this.searching,
    required this.busy,
    required this.controller,
    required this.onStart,
    required this.onChanged,
    required this.onClose,
  });

  final bool searching;
  final bool busy;
  final TextEditingController controller;
  final VoidCallback onStart;
  final ValueChanged<String> onChanged;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    if (!searching) {
      return Align(
        alignment: Alignment.centerRight,
        child: Padding(
          padding: const EdgeInsets.only(right: 8),
          child: IconButton(
            tooltip: context.strings.searchMessages,
            onPressed: onStart,
            icon: Icon(context.torcaIcons.search),
          ),
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 8, 8, 8),
      child: Row(
        children: <Widget>[
          Expanded(
            child: TextField(
              controller: controller,
              autofocus: true,
              decoration: InputDecoration(
                isDense: true,
                hintText: context.strings.searchConversationHint,
                prefixIcon: Icon(context.torcaIcons.search),
                suffixIcon: controller.text.isEmpty
                    ? null
                    : IconButton(
                        tooltip: context.strings.clearSearch,
                        onPressed: () {
                          controller.clear();
                          onChanged('');
                        },
                        icon: Icon(context.torcaIcons.close),
                      ),
              ),
              onChanged: onChanged,
            ),
          ),
          if (busy)
            const Padding(
              padding: EdgeInsets.all(10),
              child: SizedBox(
                width: 18,
                height: 18,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
            )
          else
            IconButton(
              tooltip: context.strings.closeSearch,
              onPressed: onClose,
              icon: Icon(context.torcaIcons.close),
            ),
        ],
      ),
    );
  }
}
