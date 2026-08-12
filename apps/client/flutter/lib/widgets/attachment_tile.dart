import 'dart:io';

import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';

class AttachmentTile extends StatelessWidget {
  const AttachmentTile({
    required this.attachment,
    required this.onRetry,
    required this.onCancel,
    required this.onOpen,
    required this.onSave,
    this.loadPreview,
    this.operationBusy = false,
    super.key,
  });

  final AttachmentDto attachment;
  final VoidCallback onRetry;
  final VoidCallback onCancel;
  final VoidCallback onOpen;
  final VoidCallback onSave;
  final bool operationBusy;
  final Future<String?> Function()? loadPreview;

  @override
  Widget build(BuildContext context) {
    final total = attachment.size <= 0 ? 1 : attachment.size;
    final transferred = attachment.offset.clamp(0, total);
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
          if (attachment.mediaType.startsWith('image/')) ...<Widget>[
            _AttachmentImagePreview(
              attachmentId: attachment.id,
              loadPreview: available ? loadPreview : null,
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
                      '${formatBytes(attachment.size)} · ${_statusLabel(attachment.status, attachment.direction)}'
                      '${attachment.attemptCount > 0 ? ' · attempt ${attachment.attemptCount}' : ''}'
                      '${attachment.lastErrorCode == null ? '' : ' · ${_failureLabel(attachment.lastErrorCode!)}'}',
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
              available
                  ? formatBytes(attachment.size)
                  : '${formatBytes(transferred)} / ${formatBytes(attachment.size)}',
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

  static String _statusLabel(String status, String direction) =>
      switch (status) {
        'prepared' => 'Preparing secure copy',
        'encrypting' => 'Encrypting',
        'queued' =>
          direction == 'inbound' ? 'Waiting to receive' : 'Waiting for peer',
        'transferring' || 'sending' =>
          direction == 'inbound' ? 'Receiving securely' : 'Sending securely',
        'receiving' => 'Receiving securely',
        'available' => 'Verified on device',
        'failed' => 'Transfer failed',
        'cancelled' => 'Cancelled',
        _ => status,
      };

  static String _failureLabel(String code) => switch (code) {
    'ATTACHMENT_ACK_TIMEOUT' => 'waiting for peer acknowledgement',
    'ATTACHMENT_PEER_UNAVAILABLE' => 'peer unavailable',
    'ATTACHMENT_INTEGRITY_FAILED' => 'integrity check failed',
    'ATTACHMENT_STORAGE_FAILED' => 'local storage failed',
    'ATTACHMENT_MESSAGE_PENDING' => 'waiting for message',
    'ATTACHMENT_DEPENDENCY_MISSING' => 'waiting for conversation',
    _ => 'retry available',
  };
}

class _AttachmentImagePreview extends StatefulWidget {
  const _AttachmentImagePreview({
    required this.attachmentId,
    required this.loadPreview,
  });

  final String attachmentId;
  final Future<String?> Function()? loadPreview;

  @override
  State<_AttachmentImagePreview> createState() =>
      _AttachmentImagePreviewState();
}

class _AttachmentImagePreviewState extends State<_AttachmentImagePreview> {
  Future<String?>? _path;

  @override
  void initState() {
    super.initState();
    _path = widget.loadPreview?.call();
  }

  @override
  void didUpdateWidget(covariant _AttachmentImagePreview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.attachmentId != widget.attachmentId ||
        (oldWidget.loadPreview == null && widget.loadPreview != null)) {
      _path = widget.loadPreview?.call();
    }
  }

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: 128,
    child: DecoratedBox(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        border: Border.all(color: Theme.of(context).colorScheme.outlineVariant),
      ),
      child: _path == null
          ? Icon(context.torcaIcons.image, size: 32)
          : FutureBuilder<String?>(
              future: _path,
              builder: (context, snapshot) {
                final path = snapshot.data;
                if (path == null)
                  return Icon(context.torcaIcons.image, size: 32);
                return ClipRect(
                  child: Image.file(
                    File(path),
                    width: 128,
                    height: 128,
                    fit: BoxFit.cover,
                    errorBuilder: (_, _, _) =>
                        Icon(context.torcaIcons.image, size: 32),
                  ),
                );
              },
            ),
    ),
  );
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
