SELECT g.genome_hash, g.schema_version, g.generator_version,
       g.catalog_version, g.compressed_genome
FROM local_avatar_genome local
JOIN avatar_genomes g ON g.genome_hash = local.genome_hash
WHERE local.singleton = 1
LIMIT 1;
