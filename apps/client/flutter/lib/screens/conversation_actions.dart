part of 'conversation_screen.dart';

// These methods are split into a part file to keep the screen maintainable;
// they still execute on the State instance and therefore legitimately use
// Flutter's protected State APIs.
// ignore_for_file: invalid_use_of_protected_member
extension on _ConversationPaneState {
  Future<void> _queueVoiceClip(String path, String originalName) async {
    final source = File(path);
    try {
      final capabilities = capabilitiesFor(widget.gateway);
      final prepared = await _attachmentProcessor.prepare(
        sourcePath: path,
        originalName: originalName,
        extension: 'm4a',
        maximumBytes: capabilities.maxAttachmentBytes,
      );
      if (!mounted) {
        await prepared.dispose();
        return;
      }
      final pending = _PendingAttachment(originalName, prepared);
      setState(() => _pendingAttachments.add(pending));
      await _queuePendingAttachments();
    } on AttachmentSizeException catch (error) {
      if (mounted) {
        _showError(
          '$originalName: maximum size is ${formatBytes(error.maximumBytes)}',
        );
      }
    } on Object {
      if (mounted) _showError(context.strings.couldNotQueueAttachment);
    } finally {
      if (await source.exists()) await source.delete();
    }
  }

  Future<void> _sendMessage() async {
    final body = _controller.text.trim();
    if ((body.isEmpty && _pendingAttachments.isEmpty) || _searching) return;
    if (body.characters.length > maxMessageCharacters) {
      _showError(context.strings.messageTooLong(maxMessageCharacters));
      return;
    }
    if (body.isNotEmpty) {
      final editing = _editingMessage;
      final replyTo = _replyingTo?.id;
      var sent = false;
      await _operations.run('message:send', () async {
        final result = await widget.gateway.execute(
          editing == null
              ? QueueMessageCommandDto(
                  conversationIdHex: widget.conversation.id,
                  body: body,
                  replyToMessageId: replyTo,
                )
              : EditMessageCommandDto(messageIdHex: editing.id, body: body),
        );
        if (!mounted) return;
        if (result.ok) {
          sent = true;
          _controller.clear();
          _drafts.remove(widget.conversation.id);
          final preferences = widget.preferences;
          if (preferences != null) {
            unawaited(preferences.clearDraft(widget.conversation.id));
          }
          setState(() {
            _replyingTo = null;
            _editingMessage = null;
          });
          await _timeline.refreshLatest();
          WidgetsBinding.instance.addPostFrameCallback(
            (_) => _scrollToBottom(),
          );
        } else {
          _showError(
            BridgeErrorPresenter.localized(
              context,
              result,
              fallback: 'Could not queue message',
            ),
          );
        }
      });
      if (!sent) return;
    }
    if (_pendingAttachments.isNotEmpty) await _queuePendingAttachments();
  }

  Future<void> _pickAttachments() async {
    await _operations.run('attachment:pick', () async {
      final picked = await FilePicker.pickFiles();
      if (picked.isEmpty || !mounted) return;
      final capabilities = capabilitiesFor(widget.gateway);
      final maxBytes = capabilities.maxAttachmentBytes;
      final maximumFiles = capabilities.maxQueuedAttachments;
      final maximumVideoBytes = capabilities.maxVideoAttachmentBytes;
      final remainingSlots = maximumFiles - _pendingAttachments.length;
      if (remainingSlots <= 0) {
        _showError('You can queue at most $maximumFiles attachments.');
        return;
      }
      if (picked.length > remainingSlots) {
        _showError('Only $remainingSlots attachment slots remain.');
      }
      final preparedAttachments = <_PendingAttachment>[];
      for (final file in picked.take(remainingSlots)) {
        final size = await file.length();
        if (size <= 0) {
          _showError('${file.name}: the selected file is empty');
          continue;
        }
        if (size > capabilities.maxAttachmentSourceBytes) {
          _showError(
            '${file.name}: maximum source size is '
            '${formatBytes(capabilities.maxAttachmentSourceBytes)}',
          );
          continue;
        }
        String? cleanupPath;
        var path = file.path;
        if (path == null || path.isEmpty) {
          final staged = File(
            '${Directory.systemTemp.path}${Platform.pathSeparator}'
            'torca-picked-${DateTime.now().microsecondsSinceEpoch}-'
            '${file.name.replaceAll(RegExp(r'[^A-Za-z0-9._-]'), '_')}',
          );
          try {
            final sink = staged.openWrite();
            try {
              await sink.addStream(file.readAsByteStream());
            } finally {
              await sink.close();
            }
            path = staged.path;
            cleanupPath = staged.path;
          } on Object {
            _showError('${file.name}: could not read the selected file');
            if (await staged.exists()) await staged.delete();
            continue;
          }
        }
        PreparedAttachment prepared;
        try {
          prepared = await _attachmentProcessor.prepare(
            sourcePath: path,
            originalName: file.name,
            extension: _fileExtension(file.name),
            maximumBytes: maxBytes,
            maximumVideoBytes: maximumVideoBytes,
            videoPreviewExtractor: VideoThumbnailService.extract,
          );
        } on AttachmentSizeException catch (error) {
          _showError(
            '${file.name}: maximum size is '
            '${formatBytes(error.maximumBytes)}',
          );
          continue;
        } on AttachmentSelectionException catch (error) {
          _showError('${file.name}: ${error.message}');
          continue;
        } catch (_) {
          _showError('${file.name}: the file could not be processed');
          continue;
        } finally {
          if (cleanupPath != null) {
            final staged = File(cleanupPath);
            if (await staged.exists()) await staged.delete();
          }
        }
        final limit = prepared.kind == AttachmentMediaKind.video
            ? maximumVideoBytes
            : maxBytes;
        if (prepared.size > limit) {
          _showError(
            '${file.name}: maximum ${prepared.kind == AttachmentMediaKind.video ? 'video' : 'attachment'} size is ${formatBytes(limit)}',
          );
          await prepared.dispose();
          continue;
        }
        preparedAttachments.add(_PendingAttachment(file.name, prepared));
      }
      if (preparedAttachments.isEmpty || !mounted) return;
      setState(() => _pendingAttachments.addAll(preparedAttachments));
    });
  }

  Future<void> _queuePendingAttachments() async {
    if (_pendingAttachments.isEmpty) return;
    final pending = List<_PendingAttachment>.of(_pendingAttachments);
    var queued = 0;
    await _operations.run('attachment:send', () async {
      for (final item in pending) {
        final prepared = item.prepared;
        final response = await widget.gateway.execute(
          QueueAttachmentCommandDto(
            conversationIdHex: widget.conversation.id,
            sourcePath: prepared.path,
            previewSourcePath: prepared.previewPath,
            name: prepared.name,
            mediaType: prepared.mediaType,
            size: prepared.size,
          ),
        );
        if (!mounted) return;
        if (!response.ok) {
          _showError(
            '${item.originalName}: ${BridgeErrorPresenter.localized(context, response, fallback: context.strings.couldNotQueueAttachment)}',
          );
          continue;
        }
        await prepared.dispose();
        if (!mounted) return;
        setState(() => _pendingAttachments.remove(item));
        queued++;
      }
      if (queued > 0) await _timeline.refreshLatest();
      if (mounted && queued > 1) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(context.strings.attachmentsQueued(queued))),
        );
      }
    });
  }

  String? _fileExtension(String name) {
    final dot = name.lastIndexOf('.');
    if (dot <= 0 || dot == name.length - 1) return null;
    return name.substring(dot + 1);
  }

  Future<void> _saveAttachment(AttachmentDto attachment) async {
    await _operations.run('attachment:${attachment.id}:save', () async {
      final temp = File(
        '${Directory.systemTemp.path}${torcaPathSeparator}'
        'torca-export-${DateTime.now().microsecondsSinceEpoch}-'
        '${attachment.name.replaceAll(RegExp(r'[^A-Za-z0-9._-]'), '_')}',
      );
      try {
        final result = await widget.gateway.execute(
          ExportAttachmentCommandDto(
            attachmentIdHex: attachment.id,
            destinationPath: temp.path,
          ),
        );
        if (!result.ok || !await temp.exists()) {
          if (mounted) {
            _showError(
              BridgeErrorPresenter.localized(
                context,
                result,
                fallback: 'Could not save attachment',
              ),
            );
          }
          return;
        }
        final bytes = await temp.readAsBytes();
        final destination = await FilePicker.saveFile(
          dialogTitle: context.strings.saveAttachment,
          fileName: attachment.name,
          bytes: bytes,
          mimeType: attachment.mediaType,
        );
        if (mounted && destination != null) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(context.strings.attachmentSaved)),
          );
        }
      } finally {
        if (await temp.exists()) await temp.delete();
      }
    });
  }

  Future<String?> _loadAttachmentPreview(AttachmentDto attachment) async {
    final path =
        '${Directory.systemTemp.path}${torcaPathSeparator}torca-preview-${attachment.id}.jpg';
    final file = File(path);
    if (await file.exists() && await file.length() > 0) return path;
    final preview = await widget.gateway.execute(
      ExportAttachmentPreviewCommandDto(
        attachmentIdHex: attachment.id,
        destinationPath: path,
      ),
    );
    if (preview.ok && await file.exists()) return path;
    if (attachment.typedStatus != AttachmentStatus.available) return null;
    final result = await widget.gateway.execute(
      ExportAttachmentCommandDto(
        attachmentIdHex: attachment.id,
        destinationPath: path,
      ),
    );
    return result.ok && await file.exists() ? path : null;
  }

  Future<void> _previewAttachment(AttachmentDto attachment) async {
    final path = await _loadAttachmentPreview(attachment);
    if (!mounted || path == null) {
      if (mounted) _showError('Could not load image preview');
      return;
    }
    await showDialog<void>(
      context: context,
      builder: (context) => Dialog(
        clipBehavior: Clip.antiAlias,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 960, maxHeight: 760),
          child: Stack(
            children: <Widget>[
              Positioned.fill(
                child: InteractiveViewer(
                  minScale: 0.5,
                  maxScale: 5,
                  child: Center(child: Image.file(File(path))),
                ),
              ),
              Positioned(
                top: 8,
                right: 8,
                child: IconButton.filledTonal(
                  tooltip: context.strings.close,
                  onPressed: () => Navigator.of(context).pop(),
                  icon: Icon(context.torcaIcons.close),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _openAttachment(AttachmentDto attachment) async {
    await _operations.run('attachment:${attachment.id}:open', () async {
      final ext =
          contentExtension(attachment.mediaType) ??
          safeExtension(attachment.name);
      final path =
          '${Directory.systemTemp.path}${torcaPathSeparator}torca-${attachment.id}$ext';
      final result = await widget.gateway.execute(
        ExportAttachmentCommandDto(
          attachmentIdHex: attachment.id,
          destinationPath: path,
        ),
      );
      if (!mounted) return;
      if (!result.ok) {
        _showError(
          BridgeErrorPresenter.localized(
            context,
            result,
            fallback: 'Could not open attachment',
          ),
        );
        return;
      }
      final opened = await OpenFilex.open(path);
      if (mounted && opened.type != ResultType.done) _showError(opened.message);
    });
  }

  Future<void> _attachmentCommand(
    String attachmentId,
    String action,
    BridgeCommandDto command,
  ) async {
    await _operations.run('attachment:$attachmentId:$action', () async {
      final result = await widget.gateway.execute(command);
      if (mounted && !result.ok) {
        _showError(
          BridgeErrorPresenter.localized(
            context,
            result,
            fallback: context.strings.attachmentOperationFailed,
          ),
        );
      }
    });
  }

  Future<void> _forwardMessage(MessageDto message) async {
    final snapshot = widget.gateway.snapshots.value;
    final options = snapshot.conversations
        .where((conversation) => conversation.id != widget.conversation.id)
        .toList(growable: false);
    if (!mounted || options.isEmpty) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(context.strings.chooseConversation)),
        );
      }
      return;
    }
    final target = await showDialog<ConversationDto>(
      context: context,
      builder: (context) => SimpleDialog(
        title: Text(context.strings.chooseConversation),
        children: options
            .map(
              (conversation) => SimpleDialogOption(
                onPressed: () => Navigator.of(context).pop(conversation),
                child: Text(
                  contactForSnapshot(snapshot, conversation)?.displayName ??
                      context.strings.contactLabel,
                ),
              ),
            )
            .toList(growable: false),
      ),
    );
    if (!mounted || target == null) return;
    final result = await widget.gateway.execute(
      QueueMessageCommandDto(conversationIdHex: target.id, body: message.body),
    );
    if (!mounted || result.ok) return;
    _showError(
      BridgeErrorPresenter.localized(
        context,
        result,
        fallback: 'Could not forward message',
      ),
    );
  }

  void _showError(String text) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
  }

  Widget _emojiWrap(BuildContext context, Iterable<String> values) => Wrap(
    alignment: WrapAlignment.center,
    spacing: 8,
    runSpacing: 8,
    children: values
        .map(
          (value) => IconButton(
            tooltip: value,
            icon: Text(value, style: const TextStyle(fontSize: 28)),
            onPressed: () => Navigator.of(context).pop(value),
          ),
        )
        .toList(growable: false),
  );

  Widget _detail(String label, String value) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Text('$label: $value'),
  );

  String _date(int ms) => ms <= 0
      ? context.strings.unavailable
      : DateTime.fromMillisecondsSinceEpoch(ms).toLocal().toString();
}
