-- Add migration script here
CREATE TABLE csr_challenges (
    -- Use the hash as the lookup key
    token_hash BYTEA PRIMARY KEY, 
    company_id TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '5 minutes'),
    used_at TIMESTAMPTZ
);

CREATE INDEX idx_csr_expiry ON csr_challenges(expires_at) WHERE used_at IS NULL;