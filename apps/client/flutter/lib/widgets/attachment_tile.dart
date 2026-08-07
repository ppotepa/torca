import 'package:flutter/material.dart';

import '../generated/torca_contract.dart';

class AttachmentTile extends StatelessWidget {
  const AttachmentTile({
    required this.attachment,
    required this.onRetry,
    required this.onCancel,
    required this.onOpen,
    required this.onSave,
    super.key,
  });

  final AttachmentDto attachment;
  final VoidCallback onRetry;
  final VoidCallback onCancel;
  final VoidCallback onOpen;
  final VoidCallback onSave;

  @override
  Widget build(BuildContext context) {
    final total = attachment.size <= 0 ? 1 : attachment.size;
    final transferred = attachment.offset.clamp(0, total);
    final progress = (transferred / total).clamp(0.0, 1.0);
    final failed = attachment.status == 'failed';
    final available = attachment.status == 'available';
    final cancelled = attachment.status == 'cancelled';
    final terminal = available || cancelled;

    return Container(
      margin: const EdgeInsets.only(top: 8),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.55),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(_iconFor(attachment.mediaType), size: 22),
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
                      '${formatBytes(attachment.size)} · ${_statusLabel(attachment.status)}',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ],
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
                  onPressed: onOpen,
                  icon: const Icon(Icons.open_in_new),
                  label: const Text('Open'),
                ),
              if (available)
                TextButton.icon(
                  onPressed: onSave,
                  icon: const Icon(Icons.save_alt),
                  label: const Text('Save as'),
                ),
              if (failed)
                TextButton.icon(
                  onPressed: onRetry,
                  icon: const Icon(Icons.refresh),
                  label: const Text('Retry'),
                ),
              if (!terminal)
                TextButton.icon(
                  onPressed: onCancel,
                  icon: const Icon(Icons.close),
                  label: const Text('Cancel'),
                ),
            ],
          ),
        ],
      ),
    );
  }

  static IconData _iconFor(String mediaType) {
    if (mediaType.startsWith('image/')) return Icons.image_outlined;
    if (mediaType.startsWith('video/')) return Icons.movie_outlined;
    if (mediaType.startsWith('audio/')) return Icons.audio_file_outlined;
    if (mediaType == 'application/pdf') return Icons.picture_as_pdf_outlined;
    if (mediaType.startsWith('text/')) return Icons.description_outlined;
    return Icons.insert_drive_file_outlined;
  }

  static String _statusLabel(String status) => switch (status) {
        'queued' => 'Queued',
        'preparing' => 'Preparing',
        'sending' => 'Sending',
        'receiving' => 'Receiving',
        'available' => 'Available',
        'failed' => 'Transfer failed',
        'cancelled' => 'Cancelled',
        _ => status,
      };
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
