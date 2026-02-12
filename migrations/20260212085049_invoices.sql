-- Add migration script here
CREATE INDEX idx_invoices_lookup ON invoices (company, created_at DESC);
commit;