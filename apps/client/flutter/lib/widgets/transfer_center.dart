import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';
import '../localization/torca_strings.dart';

/// Compact global view of attachment jobs. Conversation cards remain the
/// primary interaction surface; this gives users one place to find transfers
/// that continue while they navigate elsewhere.
class TransferCenterButton extends StatelessWidget {
  const TransferCenterButton({
    required this.gateway,
    required this.attachments,
    super.key,
  });

  final EngineGateway gateway;
  final List<AttachmentDto> attachments;

  @override
  Widget build(BuildContext context) {
    final active = attachments
        .where(
          (item) => switch (item.typedStatus) {
            AttachmentStatus.prepared ||
            AttachmentStatus.encrypting ||
            AttachmentStatus.queued ||
            AttachmentStatus.transferring => true,
            _ => false,
          },
        )
        .length;
    final failed = attachments
        .where((item) => item.typedStatus == AttachmentStatus.failed)
        .length;
    final count = active + failed;
    return IconButton(
      tooltip: context.strings.transfers,
      onPressed: () => _show(context),
      icon: Badge(
        isLabelVisible: count > 0,
        label: Text('$count'),
        child: Icon(context.torcaIcons.attachment),
      ),
    );
  }

  Future<void> _show(BuildContext context) => showDialog<void>(
    context: context,
    builder: (_) =>
        _TransferCenterDialog(gateway: gateway, attachments: attachments),
  );
}

class _TransferCenterDialog extends StatelessWidget {
  const _TransferCenterDialog({
    required this.gateway,
    required this.attachments,
  });

  final EngineGateway gateway;
  final List<AttachmentDto> attachments;

  @override
  Widget build(BuildContext context) {
    final visible = attachments
        .where((item) => item.typedStatus != AttachmentStatus.available)
        .toList(growable: false);
    return AlertDialog(
      title: Text(context.strings.transfers),
      content: SizedBox(
        width: 460,
        child: visible.isEmpty
            ? Text(context.strings.noActiveTransfers)
            : ListView.separated(
                shrinkWrap: true,
                itemCount: visible.length,
                separatorBuilder: (_, _) => const Divider(height: 16),
                itemBuilder: (context, index) =>
                    _TransferRow(gateway: gateway, attachment: visible[index]),
              ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(context.strings.close),
        ),
      ],
    );
  }
}

class _TransferRow extends StatelessWidget {
  const _TransferRow({required this.gateway, required this.attachment});

  final EngineGateway gateway;
  final AttachmentDto attachment;

  @override
  Widget build(BuildContext context) {
    final total = attachment.size <= 0 ? 1 : attachment.size;
    final progress = (attachment.offset / total).clamp(0.0, 1.0);
    final failed = attachment.typedStatus == AttachmentStatus.failed;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Row(
          children: <Widget>[
            Icon(_icon(context, attachment.mediaType), size: 20),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                attachment.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            Text('${(progress * 100).round()}%'),
          ],
        ),
        const SizedBox(height: 6),
        LinearProgressIndicator(value: failed ? 0 : progress),
        const SizedBox(height: 6),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: <Widget>[
            if (failed)
              TextButton.icon(
                onPressed: () => gateway.execute(
                  RetryAttachmentCommandDto(attachmentIdHex: attachment.id),
                ),
                icon: Icon(context.torcaIcons.retry, size: 16),
                label: Text(context.strings.retryNow),
              ),
            TextButton.icon(
              onPressed: () => gateway.execute(
                CancelAttachmentCommandDto(attachmentIdHex: attachment.id),
              ),
              icon: Icon(context.torcaIcons.close, size: 16),
              label: Text(context.strings.cancel),
            ),
          ],
        ),
      ],
    );
  }

  IconData _icon(BuildContext context, String mediaType) {
    if (mediaType.startsWith('image/')) return context.torcaIcons.image;
    if (mediaType.startsWith('video/')) return context.torcaIcons.video;
    if (mediaType.startsWith('audio/')) return context.torcaIcons.audio;
    return context.torcaIcons.file;
  }
}
