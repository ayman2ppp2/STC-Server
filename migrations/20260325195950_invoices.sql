-- Add migration script here
BEGIN;

-- 1. Add the new invoice_type column.
-- We use a CHECK constraint to ensure only valid types are inserted.
-- Note: I've added a DEFAULT 'reporting' so existing rows aren't left null, 
-- but you should change this to 'clearance' if that represents your legacy data better.
ALTER TABLE invoices 
    ADD COLUMN invoice_type TEXT DEFAULT 'reporting' 
    CHECK (invoice_type IN ('reporting', 'clearance'));

-- 2. Drop the old lookup index that only partitioned by company
DROP INDEX IF EXISTS idx_invoices_lookup;

-- 3. Re-create the lookup index to include the invoice_type.
-- This ensures that when your app queries for the latest invoice to get the PIH, 
-- it can efficiently filter by both company AND invoice_type.
CREATE INDEX idx_invoices_lookup ON invoices (company, invoice_type, created_at DESC);

COMMIT;