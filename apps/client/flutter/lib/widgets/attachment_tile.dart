import 'dart:io';

import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';

class AttachmentPendingTile extends StatelessWidget {
  const AttachmentPendingTile({
    required this.name,
    required this.outbound,
    super.key,
  });

  final String name;
  final bool outbound;

  @override
  Widget build(BuildContext context) => Container(
    margin: const EdgeInsets.only(top: 8),
    padding: const EdgeInsets.all(10),
    decoration: BoxDecoration(
      color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.55),
      borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
      border: context.torcaTokens.terminal
          ? Border.all(color: Theme.of(context).colorScheme.outline)
          : null,
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Row(
          children: <Widget>[
            Icon(context.torcaIcons.file, size: 22),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(name, maxLines: 1, overflow: TextOverflow.ellipsis),
                  const SizedBox(height: 2),
                  Text(
                    outbound
                        ? context.strings.preparingUpload
                        : context.strings.preparingDownload,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        const LinearProgressIndicator(),
      ],
    ),
  );
}

class AttachmentTile extends StatelessWidget {
  const AttachmentTile({
    required this.attachment,
    required this.onRetry,
    required this.onCancel,
    required this.onOpen,
    required this.onSave,
    this.onPreview,
    this.loadPreview,
    this.operationBusy = false,
    super.key,
  });

  final AttachmentDto attachment;
  final VoidCallback onRetry;
  final VoidCallback onCancel;
  final VoidCallback onOpen;
  final VoidCallback onSave;

  /// Opens an in-app visual preview.  It deliberately remains separate from
  /// [onOpen], which delegates the fully materialized file to the operating
  /// system's default application.
  final VoidCallback? onPreview;
  final bool operationBusy;
  final Future<String?> Function()? loadPreview;

  @override
  Widget build(BuildContext context) {
    final total = attachment.size <= 0 ? 1 : attachment.size;
    final transferred = attachment.offset.clamp(0, total).toInt();
    final progress = (transferred / total).clamp(0.0, 1.0);
    final failed = attachment.typedStatus == AttachmentStatus.failed;
    final available = attachment.typedStatus == AttachmentStatus.available;
    final cancelled = attachment.typedStatus == AttachmentStatus.cancelled;
    final terminal = available || cancelled;

    return Container(
      margin: const EdgeInsets.only(top: 8),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.55),
        borderRadius: BorderRadius.circular(context.torcaTokens.radiusMedium),
        border: context.torcaTokens.terminal
            ? Border.all(color: Theme.of(context).colorScheme.outline)
            : null,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          if (_hasVisualPreview(attachment.mediaType)) ...<Widget>[
            _AttachmentVisualPreview(
              attachmentId: attachment.id,
              mediaType: attachment.mediaType,
              revision: attachment.updatedAtMs,
              // Preview metadata is available independently of the complete
              // attachment payload, so receiver-side cards can render it
              // while chunks are still syncing.
              loadPreview: loadPreview,
              onTap: onPreview,
            ),
            const SizedBox(height: 8),
          ],
          Row(
            children: <Widget>[
              Icon(_iconFor(context, attachment.mediaType), size: 22),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      attachment.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      '${attachment.mediaType} / ${formatBytes(attachment.size)}',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ],
                ),
              ),
              if (operationBusy)
                const Padding(
                  padding: EdgeInsets.only(left: 8),
                  child: SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                ),
            ],
          ),
          if (!cancelled) ...<Widget>[
            const SizedBox(height: 8),
            LinearProgressIndicator(value: available ? 1 : progress),
            const SizedBox(height: 4),
            Text(
              _transferSummary(
                context,
                attachment,
                transferred: transferred,
                available: available,
              ),
              style: Theme.of(context).textTheme.labelSmall,
            ),
          ],
          Wrap(
            spacing: 6,
            runSpacing: 2,
            children: <Widget>[
              if (available)
                TextButton.icon(
                  onPressed: operationBusy ? null : onOpen,
                  icon: Icon(context.torcaIcons.open),
                  label: Text(context.strings.open),
                ),
              if (available)
                TextButton.icon(
                  onPressed: operationBusy ? null : onSave,
                  icon: Icon(context.torcaIcons.save),
                  label: Text(context.strings.saveAs),
                ),
              if (failed)
                TextButton.icon(
                  onPressed: operationBusy ? null : onRetry,
                  icon: Icon(context.torcaIcons.retry),
                  label: Text(context.strings.retryNow),
                ),
              if (!terminal)
                TextButton.icon(
                  onPressed: operationBusy ? null : onCancel,
                  icon: Icon(context.torcaIcons.close),
                  label: Text(context.strings.cancel),
                ),
            ],
          ),
        ],
      ),
    );
  }

  static IconData _iconFor(BuildContext context, String mediaType) {
    if (mediaType.startsWith('image/')) return context.torcaIcons.image;
    if (mediaType.startsWith('video/')) return context.torcaIcons.video;
    if (mediaType.startsWith('audio/')) return context.torcaIcons.audio;
    if (mediaType == 'application/pdf') return context.torcaIcons.pdf;
    if (mediaType.startsWith('text/') || mediaType == 'application/json') {
      return context.torcaIcons.textFile;
    }
    if (mediaType.contains('zip') || mediaType.contains('gzip')) {
      return context.torcaIcons.archive;
    }
    if (mediaType.contains('word') ||
        mediaType.contains('excel') ||
        mediaType.contains('powerpoint') ||
        mediaType.contains('officedocument')) {
      return context.torcaIcons.document;
    }
    return context.torcaIcons.file;
  }

  static bool _hasVisualPreview(String mediaType) =>
      mediaType.startsWith('image/') || mediaType.startsWith('video/');

  static String _statusLabel(
    BuildContext context,
    String status,
    String direction,
  ) =>
      switch (status) {
        'prepared' => context.strings.preparingSecureCopy,
        'encrypting' => context.strings.encrypting,
        'queued' =>
          direction == 'inbound'
              ? context.strings.waitingToReceive
              : context.strings.waitingForPeer,
        'transferring' || 'sending' =>
          direction == 'inbound'
              ? context.strings.receivingSecurely
              : context.strings.sendingSecurely,
        'receiving' => context.strings.receivingSecurely,
        // A message receipt may reach the projection before its attachment
        // state refresh. Keep delivery wording in MessageBubble's footer;
        // this card describes file availability only.
        'delivered' => context.strings.attachmentSyncing,
        'available' => context.strings.verifiedOnDevice,
        'failed' => context.strings.transferFailed,
        'cancelled' => context.strings.cancelled,
        _ => status,
      };

  static String _transferSummary(
    BuildContext context,
    AttachmentDto attachment, {
    required int transferred,
    required bool available,
  }) {
    final status = _statusLabel(context, attachment.status, attachment.direction);
    final progress = available
        ? formatBytes(attachment.size)
        : '${formatBytes(transferred)} / ${formatBytes(attachment.size)}';
    final attempt = attachment.attemptCount > 0
        ? ' / attempt ${attachment.attemptCount}'
        : '';
    final failure = attachment.lastErrorCode == null
        ? ''
        : ' / ${_failureLabel(context, attachment.lastErrorCode!)}';
    return '$status / $progress$attempt$failure';
  }

  static String _failureLabel(BuildContext context, String code) => switch (code) {
    'ATTACHMENT_ACK_TIMEOUT' => context.strings.attachmentAckTimeout,
    'ATTACHMENT_PEER_UNAVAILABLE' => context.strings.attachmentPeerUnavailable,
    'ATTACHMENT_INTEGRITY_FAILED' => context.strings.attachmentIntegrityFailed,
    'ATTACHMENT_STORAGE_FAILED' => context.strings.attachmentStorageFailed,
    'ATTACHMENT_MESSAGE_PENDING' => context.strings.attachmentMessagePending,
    'ATTACHMENT_DEPENDENCY_MISSING' => context.strings.attachmentDependencyMissing,
    _ => context.strings.attachmentRetryAvailable,
  };
}

class _AttachmentVisualPreview extends StatefulWidget {
  const _AttachmentVisualPreview({
    required this.attachmentId,
    required this.mediaType,
    required this.revision,
    required this.loadPreview,
    this.onTap,
  });

  final String attachmentId;
  final String mediaType;
  /// Changes whenever metadata, chunk progress or preview availability is
  /// projected from the runtime. A prior `null` result must not become a
  /// permanent empty thumbnail while the transfer continues.
  final int revision;
  final Future<String?> Function()? loadPreview;
  final VoidCallback? onTap;

  @override
  State<_AttachmentVisualPreview> createState() =>
      _AttachmentVisualPreviewState();
}

class _AttachmentVisualPreviewState extends State<_AttachmentVisualPreview> {
  Future<String?>? _path;

  @override
  void initState() {
    super.initState();
    _path = widget.loadPreview?.call();
  }

  @override
  void didUpdateWidget(covariant _AttachmentVisualPreview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.attachmentId != widget.attachmentId ||
        oldWidget.revision != widget.revision ||
        (oldWidget.loadPreview == null && widget.loadPreview != null)) {
      _path = widget.loadPreview?.call();
    }
  }

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: 128,
    child: Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: widget.onTap,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surfaceContainerHighest,
            border: Border.all(
              color: Theme.of(context).colorScheme.outlineVariant,
            ),
          ),
          child: _path == null
              ? _fallback(context)
              : FutureBuilder<String?>(
                  future: _path,
                  builder: (context, snapshot) {
                    final path = snapshot.data;
                    if (path == null) {
                      return _fallback(context);
                    }
                    return ClipRect(
                      child: Image.file(
                        File(path),
                        width: 128,
                        height: 128,
                        fit: BoxFit.cover,
                        errorBuilder: (_, _, _) => _fallback(context),
                      ),
                    );
                  },
                ),
        ),
      ),
    ),
  );

  Widget _fallback(BuildContext context) =>
      widget.mediaType.startsWith('video/')
      ? Icon(context.torcaIcons.video, size: 32)
      : Icon(context.torcaIcons.image, size: 32);
}

String formatBytes(int bytes) {
  if (bytes <= 0) return '0 B';
  const units = <String>['B', 'KiB', 'MiB', 'GiB'];
  var value = bytes.toDouble();
  var unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  if (unit == 0) return '${value.toInt()} ${units[unit]}';
  return '${value >= 10 ? value.toStringAsFixed(1) : value.toStringAsFixed(2)} ${units[unit]}';
}
