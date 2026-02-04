-- Enable logical replication
-- We create an empty publication first. Tables will be added later once they are created by the application/migration.
-- This prevents startup failure because headers (tx_counter, etc.) do not exist yet.
CREATE PUBLICATION rln_publication;
