import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:torca_ui/torca_ui.dart';

import 'avatar_repository.dart';
import 'avatar_genome_codec.dart';
import 'avatar_animation.dart';

class TorcaDeviceAvatar extends StatefulWidget {
  const TorcaDeviceAvatar({
    required this.identityId,
    required this.label,
    this.envelope,
    this.size = 40,
    this.backgroundColor,
    this.foregroundColor,
    this.presentation = const AvatarActivityPresentation(
      AvatarAnimationState.smirk,
    ),
    this.stableDevice = false,
    this.focused = false,
    super.key,
  });

  final String? identityId;
  final AvatarGenomeEnvelope? envelope;
  final String label;
  final double size;
  final Color? backgroundColor;
  final Color? foregroundColor;
  final AvatarActivityPresentation presentation;
  final bool stableDevice;
  final bool focused;

  @override
  State<TorcaDeviceAvatar> createState() => _TorcaDeviceAvatarState();
}

class _TorcaDeviceAvatarState extends State<TorcaDeviceAvatar> {
  Future<AvatarSpriteSheet?>? _future;
  Future<Uint8List?>? _previewFuture;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _ensureFuture();
  }

  @override
  void didUpdateWidget(covariant TorcaDeviceAvatar oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.identityId != widget.identityId ||
        oldWidget.envelope?.genomeHash != widget.envelope?.genomeHash ||
        oldWidget.size != widget.size ||
        oldWidget.presentation.state != widget.presentation.state ||
        oldWidget.stableDevice != widget.stableDevice) {
      _future = null;
      _previewFuture = null;
      _ensureFuture();
    }
  }

  void _ensureFuture() {
    if (_future != null) return;
    final stableId = _renderIdentity;
    if (stableId == null) return;
    final repository = AvatarRepository.instance;
    final envelopeFuture = widget.envelope == null
        ? (widget.stableDevice
              ? repository.envelopeForDevice(stableId)
              : repository.envelopeForPeer(stableId))
        : Future<AvatarGenomeEnvelope>.value(widget.envelope);
    // A single-frame preview is much cheaper than a complete animation and
    // replaces initials as soon as possible. Sprite compilation follows the
    // preview so profile setup does not launch two expensive renders at once.
    _previewFuture = envelopeFuture.then(
      (envelope) => repository.imageBytes(
        identityId: stableId,
        size: widget.size > 80 ? 96 : 48,
        envelope: envelope,
      ),
    );
    _future = _previewFuture!.then(
      (_) => envelopeFuture.then(
        (envelope) => repository.spriteSheet(
          identityId: stableId,
          size: widget.size > 80 ? 96 : 48,
          animation: widget.presentation.state,
          envelope: envelope,
        ),
      ),
    );
  }

  String? get _renderIdentity {
    final identity = widget.identityId?.trim();
    if (identity != null && identity.isNotEmpty) return identity;
    // A physical-device avatar does not depend on the freshly-created Torca
    // identity. Start rendering while the first native snapshot is still
    // arriving instead of showing initials throughout profile setup.
    return widget.stableDevice ? 'local-device' : null;
  }

  @override
  Widget build(BuildContext context) {
    final stableId = _renderIdentity;
    if (stableId == null) {
      return TorcaAvatar(
        label: widget.label,
        size: widget.size,
        backgroundColor: widget.backgroundColor,
        foregroundColor: widget.foregroundColor,
      );
    }
    return FutureBuilder<AvatarSpriteSheet?>(
      key: const ValueKey<String>('torca-avatar-loader'),
      future: _future,
      builder: (context, snapshot) {
        final sheet = snapshot.data;
        if (sheet != null) {
          return _shell(
            _AnimatedSprite(
              sheet: sheet,
              size: widget.size,
              focused: widget.focused,
              animate:
                  widget.presentation.animates &&
                  AvatarFrameClock.instance.allowsAnimation(
                    focused: widget.focused,
                  ),
              reduceMotion: MediaQuery.disableAnimationsOf(context),
            ),
          );
        }
        return FutureBuilder<Uint8List?>(
          future: _previewFuture,
          builder: (context, preview) {
            final bytes = preview.data;
            return _shell(
              bytes == null
                  ? null
                  : Image.memory(
                      bytes,
                      key: const ValueKey<String>('torca-avatar-preview'),
                      width: widget.size,
                      height: widget.size,
                      fit: BoxFit.cover,
                      filterQuality: FilterQuality.none,
                      gaplessPlayback: true,
                    ),
            );
          },
        );
      },
    );
  }

  Widget _shell(Widget? child) => TorcaAvatar(
    label: widget.label,
    size: widget.size,
    backgroundColor: widget.backgroundColor,
    foregroundColor: widget.foregroundColor,
    child: child,
  );
}

class _AnimatedSprite extends StatefulWidget {
  const _AnimatedSprite({
    required this.sheet,
    required this.size,
    required this.focused,
    required this.animate,
    required this.reduceMotion,
  });

  final AvatarSpriteSheet sheet;
  final double size;
  final bool focused;
  final bool animate;
  final bool reduceMotion;

  @override
  State<_AnimatedSprite> createState() => _AnimatedSpriteState();
}

class _AnimatedSpriteState extends State<_AnimatedSprite> {
  bool _attached = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _synchronizeClock();
  }

  @override
  void didUpdateWidget(covariant _AnimatedSprite oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.focused != widget.focused && _attached) {
      AvatarFrameClock.instance.detach(focused: oldWidget.focused);
      AvatarFrameClock.instance.attach(focused: widget.focused);
    }
    _synchronizeClock();
  }

  void _synchronizeClock() {
    final shouldAttach =
        widget.animate &&
        !widget.reduceMotion &&
        TickerMode.valuesOf(context).enabled;
    if (shouldAttach == _attached) return;
    if (_attached) {
      AvatarFrameClock.instance.detach(focused: widget.focused);
    }
    _attached = shouldAttach;
    if (_attached) {
      AvatarFrameClock.instance.attach(focused: widget.focused);
    }
  }

  @override
  void dispose() {
    if (_attached) AvatarFrameClock.instance.detach(focused: widget.focused);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: AvatarFrameClock.instance,
    builder: (context, _) {
      final frame = !_attached
          ? 0
          : (AvatarFrameClock.instance.elapsedMilliseconds ~/
                    widget.sheet.frameDuration.inMilliseconds) %
                widget.sheet.frameCount;
      return ClipRect(
        child: SizedBox.square(
          dimension: widget.size,
          child: OverflowBox(
            alignment: Alignment.centerLeft,
            minWidth: widget.size * widget.sheet.frameCount,
            maxWidth: widget.size * widget.sheet.frameCount,
            child: Transform.translate(
              offset: Offset(-frame * widget.size, 0),
              child: Image.memory(
                widget.sheet.bytes,
                key: const ValueKey<String>('torca-avatar-sprite'),
                width: widget.size * widget.sheet.frameCount,
                height: widget.size,
                fit: BoxFit.fill,
                filterQuality: FilterQuality.none,
                gaplessPlayback: true,
              ),
            ),
          ),
        ),
      );
    },
  );
}
