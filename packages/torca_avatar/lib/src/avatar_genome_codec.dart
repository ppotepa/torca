import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:avatar_genome/avatar_genome.dart';
import 'package:crypto/crypto.dart';

/// Compact, versioned representation exchanged during pairing.
///
/// The payload contains the resolved genome, never rendered pixels. It is
/// deterministic, content-addressed and safe to cache on either platform.
final class AvatarGenomeEnvelope {
  const AvatarGenomeEnvelope({
    required this.schema,
    required this.generatorVersion,
    required this.catalogVersion,
    required this.genomeHash,
    required this.compressedGenome,
  });

  final int schema;
  final String generatorVersion;
  final String catalogVersion;
  final String genomeHash;
  final Uint8List compressedGenome;

  Map<String, Object> toJson() => <String, Object>{
    'schema': schema,
    'generatorVersion': generatorVersion,
    'catalogVersion': catalogVersion,
    'genomeHash': genomeHash,
    'compressedGenome': base64UrlEncode(compressedGenome),
  };

  factory AvatarGenomeEnvelope.fromJson(Map<String, Object?> json) {
    final encoded = json['compressedGenome'];
    final schema = (json['schema'] as num?)?.toInt();
    final generatorVersion = json['generatorVersion'];
    final catalogVersion = json['catalogVersion'];
    final genomeHash = json['genomeHash'];
    if (encoded is! String ||
        encoded.isEmpty ||
        schema == null ||
        generatorVersion is! String ||
        generatorVersion.isEmpty ||
        catalogVersion is! String ||
        catalogVersion.isEmpty ||
        genomeHash is! String ||
        !RegExp(r'^[0-9a-f]{64}$').hasMatch(genomeHash)) {
      throw const FormatException('Missing avatar genome');
    }
    final compressed = base64Url.decode(encoded);
    if (compressed.isEmpty || compressed.length > 32 * 1024) {
      throw const FormatException('Avatar genome payload is too large');
    }
    return AvatarGenomeEnvelope(
      schema: schema,
      generatorVersion: generatorVersion,
      catalogVersion: catalogVersion,
      genomeHash: genomeHash,
      compressedGenome: Uint8List.fromList(compressed),
    );
  }
}

final class AvatarGenomeCodec {
  const AvatarGenomeCodec._();

  static const int schema = 1;

  static AvatarGenomeEnvelope encode(AvatarGenome genome) {
    final raw = Uint8List.fromList(utf8.encode(jsonEncode(genome.toJson())));
    final compressed = Uint8List.fromList(GZipCodec().encode(raw));
    final hash = sha256.convert(raw).toString();
    return AvatarGenomeEnvelope(
      schema: schema,
      generatorVersion: genome.generatorVersion,
      catalogVersion: AvatarGenomeVersion.catalog,
      genomeHash: hash,
      compressedGenome: compressed,
    );
  }

  static AvatarGenome decode(AvatarGenomeEnvelope envelope) {
    if (envelope.schema != schema ||
        envelope.compressedGenome.length > 32 * 1024) {
      throw const FormatException('Unsupported avatar genome envelope');
    }
    final raw = Uint8List.fromList(
      GZipCodec().decode(envelope.compressedGenome),
    );
    final hash = sha256.convert(raw).toString();
    if (hash != envelope.genomeHash) {
      throw const FormatException('Avatar genome hash mismatch');
    }
    final json = jsonDecode(utf8.decode(raw));
    if (json is! Map) throw const FormatException('Invalid avatar genome');
    return AvatarGenome.fromJson(Map<String, Object?>.from(json));
  }
}
