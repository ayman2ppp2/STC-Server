-- Migration: create tables to store parsed Invoice XML
-- This schema normalizes parties, addresses and invoice lines while
-- keeping some monetary totals on the invoice for quick queries.

BEGIN;

-- tax_schemes (simple lookup table)
CREATE TABLE IF NOT EXISTS tax_schemes (
	id BIGSERIAL PRIMARY KEY,
	scheme_id TEXT UNIQUE
);

-- parties: supplier / customer
CREATE TABLE IF NOT EXISTS parties (
	id BIGSERIAL PRIMARY KEY,
	name TEXT,
	company_id TEXT,
	tax_scheme_id BIGINT REFERENCES tax_schemes(id),
	telephone TEXT,
	email TEXT
);

-- postal addresses (one party can have many addresses but usually one)
CREATE TABLE IF NOT EXISTS postal_addresses (
	id BIGSERIAL PRIMARY KEY,
	party_id BIGINT NOT NULL REFERENCES parties(id) ON DELETE CASCADE,
	street_name TEXT,
	city_name TEXT,
	country_code TEXT
);

-- invoices: top-level invoice record
CREATE TABLE IF NOT EXISTS invoices (
	id TEXT PRIMARY KEY,
	issue_date DATE NOT NULL,
	invoice_type_code TEXT,
	document_currency_code TEXT,
	supplier_party_id BIGINT REFERENCES parties(id),
	customer_party_id BIGINT REFERENCES parties(id),
	tax_total_amount NUMERIC(18,2),
	tax_total_currency TEXT,
	line_extension_amount NUMERIC(18,2),
	tax_exclusive_amount NUMERIC(18,2),
	tax_inclusive_amount NUMERIC(18,2),
	payable_amount NUMERIC(18,2),
	raw_xml JSONB, -- store original XML (or converted JSON) for traceability
	created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- invoice_lines: one or more lines per invoice
CREATE TABLE IF NOT EXISTS invoice_lines (
	id BIGSERIAL PRIMARY KEY,
	invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
	line_id TEXT, -- the cbc:ID for the line
	quantity NUMERIC(18,4),
	unit_code TEXT,
	line_extension_amount NUMERIC(18,2),
	line_currency TEXT,
	item_name TEXT,
	item_description TEXT,
	price_amount NUMERIC(18,2),
	price_currency TEXT,
	tax_amount NUMERIC(18,2),
	tax_percent NUMERIC(6,2)
);

-- tax_subtotals: optional, can link to invoice_line or invoice
CREATE TABLE IF NOT EXISTS tax_subtotals (
	id BIGSERIAL PRIMARY KEY,
	invoice_id TEXT REFERENCES invoices(id) ON DELETE CASCADE,
	invoice_line_id BIGINT REFERENCES invoice_lines(id) ON DELETE CASCADE,
	taxable_amount NUMERIC(18,2),
	tax_amount NUMERIC(18,2),
	tax_category_percent NUMERIC(6,2),
	tax_scheme_id BIGINT REFERENCES tax_schemes(id)
);

-- convenience indexes
CREATE INDEX IF NOT EXISTS idx_invoices_issue_date ON invoices(issue_date);
CREATE INDEX IF NOT EXISTS idx_invoices_supplier ON invoices(supplier_party_id);
CREATE INDEX IF NOT EXISTS idx_invoices_customer ON invoices(customer_party_id);
CREATE INDEX IF NOT EXISTS idx_invoice_lines_invoice_id ON invoice_lines(invoice_id);

COMMIT;

-- Notes:
-- 1) `invoices.id` is TEXT because the UBL invoice ID is not guaranteed to be a Postgres UUID.
--    If you prefer UUIDs, convert/cast the field before inserting and change the column type to UUID.
-- 2) `raw_xml` stores the original document (as JSON or XML converted to JSONB). It's helpful for audits.
-- 3) Monetary amounts use NUMERIC for safe decimal arithmetic. Adjust precision/scale to your domain needs.
-- 4) You can denormalize further (embed party info directly on invoices) if you prefer simpler queries.
