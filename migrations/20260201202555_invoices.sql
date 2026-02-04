-- Add migration script hereBEGIN;
-- Drop old schema
DROP TABLE IF EXISTS 
    tax_subtotals, invoice_lines, invoices, 
    postal_addresses, parties, tax_schemes 
CASCADE;

-- Create simplified table
CREATE TABLE invoices (
    uuid UUID PRIMARY KEY, -- No default; server-side generation assumed
    hash TEXT NOT NULL,
    company TEXT,
    invoiceb64 TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Ensure no two invoices can have the same hash
CREATE UNIQUE INDEX idx_invoices_hash ON invoices(hash);

COMMIT;