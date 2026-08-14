import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:avatar_genome/avatar_genome_io.dart';
import 'package:path_provider/path_provider.dart';

import 'avatar_genome_codec.dart';
import 'avatar_animation.dart';
import 'avatar_device_seed.dart';

final class AvatarSpriteSheet {
  const AvatarSpriteSheet({
    required this.bytes,
    required this.frameCount,
    required this.frameDuration,
  });

  final Uint8List bytes;
  final int frameCount;
  final Duration frameDuration;
}

final class AvatarRepository {
  AvatarRepository._();

  static final AvatarRepository instance = AvatarRepository._();
  static const int _maxMemoryEntries = 96;
  static const int _maxDiskBytes = 32 * 1024 * 1024;
  final Map<String, Uint8List> _memory = <String, Uint8List>{};
  final Map<String, AvatarGenomeEnvelope> _envelopes =
      <String, AvatarGenomeEnvelope>{};
  final Map<String, Future<AvatarGenomeEnvelope?>> _remoteEnvelopeInFlight =
      <String, Future<AvatarGenomeEnvelope?>>{};
  final Map<String, Future<Uint8List?>> _inFlight =
      <String, Future<Uint8List?>>{};
  final Map<String, AvatarSpriteSheet> _spriteMemory =
      <String, AvatarSpriteSheet>{};
  final Map<String, Future<AvatarSpriteSheet?>> _spriteInFlight =
      <String, Future<AvatarSpriteSheet?>>{};
  Directory? _cacheRoot;

  /// Application-bound loader for genomes received and authenticated during
  /// pairing. The package remains independent from the native gateway.
  Future<AvatarGenomeEnvelope?> Function(String identityId)?
  remoteEnvelopeLoader;

  /// Returns an immutable genome for a stable seed. It is generated once
  /// and persisted beside rendered cache entries; subsequent renders are
  /// content-addressed by [AvatarGenomeEnvelope.genomeHash].
  Future<AvatarGenomeEnvelope> envelopeForIdentity(String identityId) async {
    final cached = _envelopes[identityId];
    if (cached != null) return cached;
    final file = await _envelopeFile(identityId);
    if (file != null && await file.exists()) {
      try {
        final decoded = AvatarGenomeEnvelope.fromJson(
          Map<String, Object?>.from(jsonDecode(await file.readAsString())),
        );
        _envelopes[identityId] = decoded;
        return decoded;
      } on Object {
        await _deleteQuietly(file);
      }
    }
    final envelope = await Isolate.run(() => _generateEnvelope(identityId));
    _envelopes[identityId] = envelope;
    if (file != null) {
      await file.parent.create(recursive: true);
      await _writeTextAtomically(file, jsonEncode(envelope.toJson()));
    }
    return envelope;
  }

  Future<AvatarGenomeEnvelope> envelopeForDevice(
    String fallbackIdentity,
  ) async {
    final stableSeed = await AvatarDeviceSeed.resolve(
      fallbackIdentity: fallbackIdentity,
    );
    return envelopeForIdentity('device-$stableSeed');
  }

  Future<AvatarGenomeEnvelope> envelopeForPeer(String identityId) async {
    final cached = _envelopes['peer-$identityId'];
    if (cached != null) return cached;
    final remote = await _loadRemoteEnvelope(identityId);
    if (remote != null) {
      _envelopes['peer-$identityId'] = remote;
      return remote;
    }
    // Legacy contacts without an exchanged genome keep deterministic fallback
    // behavior until their next pairing/identity refresh.
    return envelopeForIdentity(identityId);
  }

  Future<AvatarGenomeEnvelope?> _loadRemoteEnvelope(String identityId) {
    final existing = _remoteEnvelopeInFlight[identityId];
    if (existing != null) return existing;
    final future = () async {
      try {
        return await remoteEnvelopeLoader?.call(identityId);
      } on Object {
        return null;
      } finally {
        _remoteEnvelopeInFlight.remove(identityId);
      }
    }();
    _remoteEnvelopeInFlight[identityId] = future;
    return future;
  }

  Future<AvatarSpriteSheet?> spriteSheet({
    required String identityId,
    required int size,
    required AvatarAnimationState animation,
    AvatarGenomeEnvelope? envelope,
  }) {
    final safeSize = AvatarRenderSettings.supportedSizes.contains(size)
        ? size
        : 48;
    final key =
        '${envelope?.genomeHash ?? 'legacy-$identityId'}/$safeSize/${animation.name}/p32/v1';
    final cached = _spriteMemory[key];
    if (cached != null) return Future<AvatarSpriteSheet?>.value(cached);
    final inFlight = _spriteInFlight[key];
    if (inFlight != null) return inFlight;
    final future = _loadOrRenderSprite(
      identityId,
      safeSize,
      key,
      animation,
      envelope,
    );
    _spriteInFlight[key] = future;
    return future.whenComplete(() => _spriteInFlight.remove(key));
  }

  Future<AvatarSpriteSheet?> _loadOrRenderSprite(
    String identityId,
    int size,
    String key,
    AvatarAnimationState animation,
    AvatarGenomeEnvelope? envelope,
  ) async {
    final file = await _cacheFile('sprites/$key');
    Uint8List? bytes;
    if (file != null && await file.exists()) {
      try {
        bytes = await file.readAsBytes();
      } on Object {
        await _deleteQuietly(file);
      }
    }
    if (bytes == null) {
      final envelopeJson = envelope?.toJson();
      try {
        bytes = await Isolate.run(
          () => _renderSprite(identityId, size, animation.name, envelopeJson),
        );
      } on Object {
        // Android can reject isolate work while the Flutter engine is still
        // entering its first resumed frame. A failed background optimization
        // must not turn into a permanent initials placeholder: render once in
        // the current isolate and cache the resulting sheet normally.
        bytes = _renderSprite(identityId, size, animation.name, envelopeJson);
      }
    }
    if (bytes == null || bytes.isEmpty) return null;
    final sheet = AvatarSpriteSheet(
      bytes: bytes,
      frameCount: animation.frameCount,
      frameDuration: animation.frameDuration,
    );
    _spriteMemory[key] = sheet;
    while (_spriteMemory.length > _maxMemoryEntries) {
      _spriteMemory.remove(_spriteMemory.keys.first);
    }
    if (file != null && !await file.exists()) {
      await _writeAtomically(file, bytes);
    }
    return sheet;
  }

  Future<Uint8List?> imageBytes({
    required String identityId,
    required int size,
    AvatarGenomeEnvelope? envelope,
  }) {
    final safeSize = AvatarRenderSettings.supportedSizes.contains(size)
        ? size
        : 48;
    final key = '${envelope?.genomeHash ?? 'legacy-$identityId'}/$safeSize';
    final cached = _memory.remove(key);
    if (cached != null) {
      _memory[key] = cached;
      return Future<Uint8List?>.value(cached);
    }
    final existing = _inFlight[key];
    if (existing != null) return existing;
    final future = _loadOrRender(identityId, safeSize, key, envelope);
    _inFlight[key] = future;
    return future.whenComplete(() => _inFlight.remove(key));
  }

  Future<Uint8List?> _loadOrRender(
    String identityId,
    int size,
    String key,
    AvatarGenomeEnvelope? envelope,
  ) async {
    final file = await _cacheFile(key);
    if (file != null && await file.exists()) {
      try {
        final bytes = await file.readAsBytes();
        if (bytes.isNotEmpty) {
          _remember(key, bytes);
          return bytes;
        }
      } on Object {
        await _deleteQuietly(file);
      }
    }
    try {
      final bytes = await Isolate.run(
        () => _renderPng(identityId, size, envelope?.toJson()),
      );
      if (bytes == null) return null;
      _remember(key, bytes);
      if (file != null) await _writeAtomically(file, bytes);
      return bytes;
    } on Object {
      // A platform isolate can be unavailable in widget tests or during an
      // early engine startup. Keep the feature usable; production Flutter
      // targets take the isolate path above.
      try {
        return _renderPng(identityId, size, envelope?.toJson());
      } on Object {
        return null;
      }
    }
  }

  Future<File?> _cacheFile(String key) async {
    try {
      final root = _cacheRoot ??= Directory(
        '${(await getApplicationCacheDirectory()).path}${Platform.pathSeparator}torca${Platform.pathSeparator}avatars${Platform.pathSeparator}v1',
      );
      final safe = key.replaceAll(RegExp(r'[^a-zA-Z0-9_./-]'), '_');
      final file = File('${root.path}${Platform.pathSeparator}$safe.png');
      await file.parent.create(recursive: true);
      return file;
    } on Object {
      return null;
    }
  }

  Future<File?> _envelopeFile(String identityId) async {
    try {
      final root = _cacheRoot ??= Directory(
        '${(await getApplicationCacheDirectory()).path}${Platform.pathSeparator}torca${Platform.pathSeparator}avatars${Platform.pathSeparator}v1',
      );
      final safe = identityId.replaceAll(RegExp(r'[^a-zA-Z0-9_.-]'), '_');
      return File('${root.path}${Platform.pathSeparator}identity-$safe.json');
    } on Object {
      return null;
    }
  }

  void _remember(String key, Uint8List bytes) {
    _memory.remove(key);
    _memory[key] = bytes;
    while (_memory.length > _maxMemoryEntries) {
      _memory.remove(_memory.keys.first);
    }
  }

  Future<void> _writeAtomically(File file, Uint8List bytes) async {
    final temporary = File(
      '${file.path}.${DateTime.now().microsecondsSinceEpoch}.tmp',
    );
    try {
      await temporary.writeAsBytes(bytes, flush: true);
      await temporary.rename(file.path);
    } on Object {
      await _deleteQuietly(temporary);
    }
  }

  Future<void> _deleteQuietly(File file) async {
    try {
      await file.delete();
    } on Object {
      // Cache failures must never affect conversation rendering.
    }
  }

  Future<void> _writeTextAtomically(File destination, String value) async {
    final temporary = File(
      '${destination.path}.tmp-${Isolate.current.hashCode}',
    );
    try {
      await temporary.writeAsString(value, flush: true);
      await temporary.rename(destination.path);
    } finally {
      if (await temporary.exists()) {
        await _deleteQuietly(temporary);
      }
    }
  }

  Future<void> trimDiskCache() async {
    final root = _cacheRoot;
    if (root == null || !await root.exists()) return;
    final files =
        (await root
                .list(recursive: true)
                .where(
                  (entity) => entity is File && entity.path.endsWith('.png'),
                )
                .toList())
            .cast<File>();
    var total = 0;
    for (final file in files) {
      total += await file.length();
    }
    if (total <= _maxDiskBytes) return;
    files.sort(
      (a, b) => a.statSync().modified.compareTo(b.statSync().modified),
    );
    for (final file in files) {
      if (total <= _maxDiskBytes) break;
      total -= await file.length();
      await _deleteQuietly(file);
    }
  }
}

Uint8List? _renderPng(
  String identityId,
  int size,
  Map<String, Object>? envelopeJson,
) {
  final generator = AvatarGenerator(cacheCapacity: 2);
  final envelope = envelopeJson == null
      ? null
      : AvatarGenomeEnvelope.fromJson(envelopeJson);
  final result = envelope == null
      ? generator.generate(
          AvatarRequest(
            seed: 'torca-device-v1:$identityId',
            overrides: const <String, Object>{'colors.colorBudget': '32'},
          ),
        )
      : () {
          final genome = AvatarGenomeCodec.decode(envelope);
          return _generatorForGenome(
            genome,
          ).generate(AvatarRequest(seed: genome.seed));
        }();
  if (!result.validation.isValid) return null;
  return AvatarPngCodec(scale: size > 48 ? 2 : 1).encode(result);
}

AvatarGenomeEnvelope _generateEnvelope(String identityId) {
  final result = AvatarGenerator(cacheCapacity: 1).generate(
    AvatarRequest(
      seed: 'torca-device-v1:$identityId',
      overrides: const <String, Object>{'colors.colorBudget': '32'},
    ),
  );
  return AvatarGenomeCodec.encode(result.genome);
}

Uint8List? _renderSprite(
  String identityId,
  int size,
  String animationName,
  Map<String, Object>? envelopeJson,
) {
  final state = AvatarAnimationState.values.byName(animationName);
  final envelope = envelopeJson == null
      ? null
      : AvatarGenomeEnvelope.fromJson(envelopeJson);
  final decodedGenome = envelope == null
      ? null
      : AvatarGenomeCodec.decode(envelope);
  final overrides = <String, Object>{
    'colors.colorBudget': '32',
    ...state.generatorOverrides,
  };
  final seed = envelope == null
      ? 'torca-device-v1:$identityId'
      : decodedGenome!.seed;
  final generator = decodedGenome == null
      ? AvatarGenerator(cacheCapacity: 2)
      : _generatorForGenome(decodedGenome, overrides: overrides);
  final animation = generator.generateAnimation(
    decodedGenome == null
        ? AvatarRequest(seed: seed, overrides: overrides)
        : AvatarRequest(seed: seed),
    frameCount: state.frameCount,
    frameDuration: state.frameDuration,
  );
  return AvatarSpriteSheetCodec(
    columns: state.frameCount,
    scale: size > 48 ? 2 : 1,
  ).encode(animation);
}

AvatarGenerator _generatorForGenome(
  AvatarGenome genome, {
  Map<String, Object> overrides = const <String, Object>{},
}) {
  // Rendering an exchanged genome must not run it through generation again.
  // Its map includes canonical and derived values; treating those as request
  // overrides fails validation and produced the permanent initials placeholder.
  final pinned = AvatarGenome(
    seed: genome.seed,
    generatorVersion: genome.generatorVersion,
    profile: genome.profile,
    values: <String, Object>{...genome.values, ...overrides},
    sources: genome.sources,
  );
  return AvatarGenerator(
    cacheCapacity: 2,
    genomeService: _PinnedGenomeGenerator(pinned),
  );
}

final class _PinnedGenomeGenerator implements GenomeGenerator {
  const _PinnedGenomeGenerator(this.genome);

  final AvatarGenome genome;

  @override
  AvatarGenome generate(AvatarRequest request, ConstraintEngine guard) =>
      genome;
}
