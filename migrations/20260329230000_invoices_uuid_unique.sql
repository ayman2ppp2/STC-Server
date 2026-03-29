-- Add unique constraint on invoices.uuid
BEGIN;

ALTER TABLE invoices ADD CONSTRAINT invoices_uuid_unique UNIQUE (uuid);

COMMIT;
