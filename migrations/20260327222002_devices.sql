-- Add migration script here
CREATE TABLE devices (
    device_uuid UUID PRIMARY KEY, -- The "SerialNumber" from CSR
    tin VARCHAR(10) NOT NULL REFERENCES taxpayers(tin),
    current_icv INTEGER NOT NULL DEFAULT 0,
    last_pih TEXT NOT NULL DEFAULT '0000000000000000000000000000000000000000000=',
    is_active BOOLEAN DEFAULT TRUE,
    onboarded_at TIMESTAMPTZ DEFAULT NOW()
);