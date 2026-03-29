-- Add migration script here
CREATE TABLE taxpayers (
    tin VARCHAR(10) PRIMARY KEY, -- The "OrganizationName" from CSR
    name TEXT NOT NULL,
    address TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);