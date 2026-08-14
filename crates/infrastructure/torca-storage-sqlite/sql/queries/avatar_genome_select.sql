SELECT schema_version, generator_version, catalog_version, compressed_genome
FROM avatar_genomes
WHERE genome_hash = ?1;
