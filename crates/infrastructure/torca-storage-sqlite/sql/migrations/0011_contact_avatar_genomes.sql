CREATE TABLE IF NOT EXISTS contact_avatar_genomes (
    contact_id BLOB PRIMARY KEY REFERENCES contacts(contact_id) ON DELETE CASCADE,
    genome_hash BLOB NOT NULL REFERENCES avatar_genomes(genome_hash),
    CHECK(length(contact_id) = 16),
    CHECK(length(genome_hash) = 32)
);

CREATE TABLE IF NOT EXISTS local_avatar_genome (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    genome_hash BLOB NOT NULL REFERENCES avatar_genomes(genome_hash),
    CHECK(length(genome_hash) = 32)
);

-- Preserve the avatar selected by pre-v11 clients before remote genomes gain
-- their own explicit contact binding.
INSERT OR IGNORE INTO local_avatar_genome(singleton, genome_hash)
SELECT 1, genome_hash
FROM avatar_genomes
ORDER BY created_at_ms DESC
LIMIT 1;

CREATE INDEX IF NOT EXISTS idx_contacts_remote_identity
    ON contacts(remote_identity_id);
