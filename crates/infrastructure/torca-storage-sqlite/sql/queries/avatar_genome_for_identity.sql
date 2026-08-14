SELECT g.genome_hash, g.schema_version, g.generator_version,
       g.catalog_version, g.compressed_genome
FROM contacts c
JOIN contact_avatar_genomes ca ON ca.contact_id = c.contact_id
JOIN avatar_genomes g ON g.genome_hash = ca.genome_hash
WHERE c.remote_identity_id = ?1
LIMIT 1;
