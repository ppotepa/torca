part of 'conversation_screen.dart';

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
        final isImage = prepared.kind == AttachmentMediaKind.image;
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
                  child: isImage
                      ? Image.file(File(prepared.path), fit: BoxFit.cover)
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
                      '${prepared.mediaType} · ${formatBytes(prepared.size)}',
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
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull => isEmpty ? null : first;
}
