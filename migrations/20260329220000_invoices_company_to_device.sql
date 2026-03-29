-- Add migration script here
BEGIN;

-- Clear existing invoices (old company values aren't valid UUIDs)
TRUNCATE TABLE invoices;

-- Rename company column to device_id and change type to UUID
ALTER TABLE invoices 
    RENAME COLUMN company TO device_id;

ALTER TABLE invoices 
    ALTER COLUMN device_id TYPE UUID USING device_id::UUID;

-- Add foreign key constraint to devices table
ALTER TABLE invoices 
    ADD CONSTRAINT fk_invoices_device 
    FOREIGN KEY (device_id) REFERENCES devices(device_uuid);

-- Update the lookup index to use device_id instead of company
DROP INDEX IF EXISTS idx_invoices_lookup;

CREATE INDEX idx_invoices_lookup ON invoices (device_id, invoice_type, created_at DESC);

COMMIT;
