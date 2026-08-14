INSERT INTO avatar_genomes(
    genome_hash, schema_version, generator_version, catalog_version,
    compressed_genome, created_at_ms
)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
ON CONFLICT(genome_hash) DO UPDATE SET
    schema_version = excluded.schema_version,
    generator_version = excluded.generator_version,
    catalog_version = excluded.catalog_version,
    compressed_genome = excluded.compressed_genome;
