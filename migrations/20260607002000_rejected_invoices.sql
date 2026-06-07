CREATE TABLE rejected_invoices (
    id UUID PRIMARY KEY,
    submitted_uuid TEXT NOT NULL,
    submitted_invoice_hash TEXT NOT NULL,
    submitted_invoice TEXT NOT NULL,
    endpoint TEXT NOT NULL CHECK (endpoint IN ('clear', 'report')),
    invoice_type TEXT NOT NULL CHECK (invoice_type IN ('clearance', 'reporting')),
    error_code TEXT NOT NULL,
    error_message TEXT NOT NULL,
    http_status INTEGER NOT NULL,
    supplier_tin TEXT,
    device_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
