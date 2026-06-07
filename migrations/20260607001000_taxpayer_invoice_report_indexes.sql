CREATE INDEX IF NOT EXISTS idx_devices_tin_device_uuid
    ON devices (tin, device_uuid);

CREATE INDEX IF NOT EXISTS idx_invoices_device_created_at
    ON invoices (device_id, created_at DESC);
