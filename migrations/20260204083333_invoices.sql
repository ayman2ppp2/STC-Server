-- Add migration script here
-- 1. Remove the old index
DROP INDEX IF EXISTS idx_invoices_hash;

-- 2. Convert the column to BYTEA
-- We use 'USING hash::bytea' if the data is already in a compatible format
ALTER TABLE invoices 
    ALTER COLUMN hash TYPE BYTEA USING hash::bytea;

-- 3. Add a constraint to ensure it is exactly 32 bytes (256 bits)
ALTER TABLE invoices 
    ADD CONSTRAINT hash_binary_length_check CHECK (octet_length(hash) = 32);

-- 4. Re-create the unique index
CREATE UNIQUE INDEX idx_invoices_hash ON invoices(hash);

COMMIT;