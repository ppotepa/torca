INSERT INTO contact_avatar_genomes(contact_id, genome_hash)
VALUES (?1, ?2)
ON CONFLICT(contact_id) DO UPDATE SET genome_hash = excluded.genome_hash;
