import 'package:flutter/material.dart';
import 'package:torca_l10n/torca_l10n.dart';
import 'package:torca_ui/torca_ui.dart';

import '../gateway/engine_gateway.dart';
import '../generated/torca_contract.dart';

/// Compact global view of attachment jobs. Conversation cards remain the
/// primary interaction surface; this gives users one place to find transfers
/// that continue while they navigate elsewhere.
class TransferCenterButton extends StatelessWidget {
  const TransferCenterButton({
    required this.gateway,
    required this.attachments,
    required this.pendingOperations,
    super.key,
  });

  final EngineGateway gateway;
  final List<AttachmentDto> attachments;
  final List<PendingOperationDto> pendingOperations;

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
    final pending = pendingOperations
        .where((item) => item.typedState != PendingOperationState.unknown)
        .length;
    final count = active + failed + pending;
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
    builder: (_) => _TransferCenterDialog(
      gateway: gateway,
      attachments: attachments,
      pendingOperations: pendingOperations,
    ),
  );
}

enum _TransferStatusFilter { all, active, completed }

enum _TransferKindFilter { all, media, documents, recordings, files }

class _TransferCenterDialog extends StatefulWidget {
  const _TransferCenterDialog({
    required this.gateway,
    required this.attachments,
    required this.pendingOperations,
  });

  final EngineGateway gateway;
  final List<AttachmentDto> attachments;
  final List<PendingOperationDto> pendingOperations;

  @override
  State<_TransferCenterDialog> createState() => _TransferCenterDialogState();
}

class _TransferCenterDialogState extends State<_TransferCenterDialog> {
  _TransferStatusFilter _statusFilter = _TransferStatusFilter.all;
  _TransferKindFilter _kindFilter = _TransferKindFilter.all;

  @override
  Widget build(BuildContext context) {
    final dialogWidth = (MediaQuery.sizeOf(context).width - 48)
        .clamp(0.0, 460.0)
        .toDouble();
    final visible = widget.attachments
        .where(
          (item) =>
              item.typedStatus != AttachmentStatus.available &&
              _matchesKind(item, _kindFilter),
        )
        .toList(growable: false);
    final completed = widget.attachments
        .where(
          (item) =>
              item.typedStatus == AttachmentStatus.available &&
              _matchesKind(item, _kindFilter),
        )
        .toList(growable: false);
    final pending = widget.pendingOperations
        .where((item) => item.typedState != PendingOperationState.unknown)
        .toList(growable: false);
    final showPending =
        _kindFilter == _TransferKindFilter.all &&
        _statusFilter != _TransferStatusFilter.completed;
    final showFiles = _statusFilter != _TransferStatusFilter.completed;
    final showCompleted = _statusFilter != _TransferStatusFilter.active;
    return AlertDialog(
      title: Text(context.strings.transfers),
      content: ConstrainedBox(
        constraints: BoxConstraints(
          minWidth: dialogWidth,
          maxWidth: dialogWidth,
          maxHeight: MediaQuery.sizeOf(context).height * 0.68,
        ),
        child:
            (!showFiles || visible.isEmpty) &&
                (!showPending || pending.isEmpty) &&
                (!showCompleted || completed.isEmpty)
            ? Text(context.strings.noActiveTransfers)
            : Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  _statusFilters(context),
                  const SizedBox(height: 8),
                  _kindFilters(context),
                  const SizedBox(height: 12),
                  Flexible(
                    child: ListView(
                      shrinkWrap: true,
                      children: <Widget>[
                        if (showPending)
                          for (final operation in pending)
                            _PendingOperationRow(operation: operation),
                        if (showPending &&
                            showFiles &&
                            pending.isNotEmpty &&
                            visible.isNotEmpty)
                          const Divider(height: 20),
                        if (showFiles)
                          for (final attachment in visible)
                            _TransferRow(
                              gateway: widget.gateway,
                              attachment: attachment,
                            ),
                        if (showCompleted && completed.isNotEmpty) ...[
                          if ((showPending && pending.isNotEmpty) ||
                              (showFiles && visible.isNotEmpty))
                            const Divider(height: 20),
                          for (final attachment in completed)
                            _TransferRow(
                              gateway: widget.gateway,
                              attachment: attachment,
                              completed: true,
                            ),
                        ],
                      ],
                    ),
                  ),
                ],
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

  Widget _statusFilters(BuildContext context) => Wrap(
    spacing: 8,
    runSpacing: 6,
    children: <Widget>[
      FilterChip(
        label: Text(context.strings.allOperations),
        selected: _statusFilter == _TransferStatusFilter.all,
        onSelected: (_) =>
            setState(() => _statusFilter = _TransferStatusFilter.all),
      ),
      FilterChip(
        label: Text(context.strings.activeTransfers),
        selected: _statusFilter == _TransferStatusFilter.active,
        onSelected: (_) =>
            setState(() => _statusFilter = _TransferStatusFilter.active),
      ),
      FilterChip(
        label: Text(context.strings.completedTransfers),
        selected: _statusFilter == _TransferStatusFilter.completed,
        onSelected: (_) =>
            setState(() => _statusFilter = _TransferStatusFilter.completed),
      ),
    ],
  );

  Widget _kindFilters(BuildContext context) => Wrap(
    spacing: 8,
    runSpacing: 6,
    children: <Widget>[
      FilterChip(
        label: Text(context.strings.allOperations),
        selected: _kindFilter == _TransferKindFilter.all,
        onSelected: (_) =>
            setState(() => _kindFilter = _TransferKindFilter.all),
      ),
      FilterChip(
        label: Text(context.strings.mediaTransfers),
        selected: _kindFilter == _TransferKindFilter.media,
        onSelected: (_) =>
            setState(() => _kindFilter = _TransferKindFilter.media),
      ),
      FilterChip(
        label: Text(context.strings.documentTransfers),
        selected: _kindFilter == _TransferKindFilter.documents,
        onSelected: (_) =>
            setState(() => _kindFilter = _TransferKindFilter.documents),
      ),
      FilterChip(
        label: Text(context.strings.recordingTransfers),
        selected: _kindFilter == _TransferKindFilter.recordings,
        onSelected: (_) =>
            setState(() => _kindFilter = _TransferKindFilter.recordings),
      ),
      FilterChip(
        label: Text(context.strings.fileTransfers),
        selected: _kindFilter == _TransferKindFilter.files,
        onSelected: (_) =>
            setState(() => _kindFilter = _TransferKindFilter.files),
      ),
    ],
  );
}

bool _matchesKind(AttachmentDto item, _TransferKindFilter filter) {
  if (filter == _TransferKindFilter.all) return true;
  final mediaType = item.mediaType.toLowerCase();
  final isMedia =
      mediaType.startsWith('image/') || mediaType.startsWith('video/');
  final isRecording = mediaType.startsWith('audio/');
  final isDocument =
      mediaType == 'application/pdf' ||
      mediaType.startsWith('text/') ||
      mediaType.contains('document') ||
      mediaType.contains('word') ||
      mediaType.contains('sheet') ||
      mediaType.contains('excel') ||
      mediaType.contains('presentation') ||
      mediaType.contains('powerpoint');
  return switch (filter) {
    _TransferKindFilter.all => true,
    _TransferKindFilter.media => isMedia,
    _TransferKindFilter.documents => isDocument,
    _TransferKindFilter.recordings => isRecording,
    _TransferKindFilter.files => !isMedia && !isRecording && !isDocument,
  };
}

class _PendingOperationRow extends StatelessWidget {
  const _PendingOperationRow({required this.operation});

  final PendingOperationDto operation;

  @override
  Widget build(BuildContext context) => ListTile(
    contentPadding: EdgeInsets.zero,
    leading: const SizedBox(
      width: 22,
      height: 22,
      child: CircularProgressIndicator(strokeWidth: 2),
    ),
    title: Text(_label(operation.kind)),
    subtitle: Text(
      _subtitle(context, operation),
      maxLines: 2,
      overflow: TextOverflow.ellipsis,
    ),
    trailing: Text('#${operation.attempts}'),
  );

  String _subtitle(BuildContext context, PendingOperationDto operation) {
    final error = operation.lastError?.trim();
    if (error?.isNotEmpty == true) return error!;
    final state = operation.typedState == PendingOperationState.retrying
        ? context.strings.retrying
        : context.strings.messageQueued;
    final dependency = switch (operation.typedDependency) {
      PendingOperationDependency.provider => 'provider',
      PendingOperationDependency.communication => 'transport',
      PendingOperationDependency.communicationAndRendezvous =>
        'transport + rendezvous',
      PendingOperationDependency.rendezvous => 'rendezvous',
      PendingOperationDependency.runtime => 'runtime',
      PendingOperationDependency.network => 'network',
      PendingOperationDependency.unknown => null,
    };
    return dependency == null
        ? state
        : '$state · ${context.strings.waitingForDependency(dependency)}';
  }

  String _label(String kind) {
    final normalized = kind.replaceAll('_', ' ');
    return normalized.isEmpty
        ? 'Pending operation'
        : normalized[0].toUpperCase() + normalized.substring(1);
  }
}

class _TransferRow extends StatelessWidget {
  const _TransferRow({
    required this.gateway,
    required this.attachment,
    this.completed = false,
  });

  final EngineGateway gateway;
  final AttachmentDto attachment;
  final bool completed;

  @override
  Widget build(BuildContext context) {
    final total = attachment.size <= 0 ? 1 : attachment.size;
    final progress = completed
        ? 1.0
        : (attachment.offset / total).clamp(0.0, 1.0);
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
            if (completed)
              Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Icon(context.torcaIcons.success, size: 16),
                  const SizedBox(width: 4),
                  Text(context.strings.completedTransfers),
                ],
              )
            else if (failed)
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
