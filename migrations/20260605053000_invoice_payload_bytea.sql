BEGIN;

ALTER TABLE invoices
    DROP CONSTRAINT IF EXISTS invoices_uuid_unique;

ALTER TABLE invoices
    RENAME COLUMN invoiceb64 TO invoice_bytes;

ALTER TABLE invoices
    ALTER COLUMN invoice_bytes TYPE BYTEA
    USING CASE
        WHEN invoice_bytes IS NULL THEN NULL
        WHEN invoice_type = 'clearance' THEN decode(invoice_bytes, 'base64')
        ELSE convert_to(invoice_bytes, 'UTF8')
    END;

COMMIT;
