INSERT INTO local_avatar_genome(singleton, genome_hash)
VALUES (1, ?1)
ON CONFLICT(singleton) DO UPDATE SET genome_hash = excluded.genome_hash;
