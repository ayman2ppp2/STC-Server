-- Add index on company_id for faster token lookup
BEGIN;

CREATE INDEX idx_csr_challenges_company_id ON csr_challenges(company_id);

COMMIT;
