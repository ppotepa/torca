CREATE TABLE IF NOT EXISTS avatar_genomes (
    genome_hash BLOB PRIMARY KEY,
    schema_version INTEGER NOT NULL,
    generator_version TEXT NOT NULL,
    catalog_version TEXT NOT NULL,
    compressed_genome BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    CHECK(length(genome_hash) = 32),
    CHECK(length(compressed_genome) > 0 AND length(compressed_genome) <= 32768)
);
