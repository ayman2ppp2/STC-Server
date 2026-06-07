# STC Server Technical Documentation

Detailed implementation and API reference for the STC electronic invoicing backend.

This document reflects the current server behavior in `src/main.rs`, route handlers, models, services, and migrations. It intentionally documents some prototype behavior, including debug endpoints and uneven sandbox header handling.

## Contents

- [Service Profile](#service-profile)
- [Configuration](#configuration)
- [Response Shapes](#response-shapes)
- [Endpoints](#endpoints)
- [Enrollment Flow](#enrollment-flow)
- [Invoice Submission](#invoice-submission)
- [Validation Pipeline](#validation-pipeline)
- [Sandbox Mode](#sandbox-mode)
- [Database Schema](#database-schema)
- [Concurrency And State](#concurrency-and-state)
- [Operational Notes](#operational-notes)
- [Examples](#examples)

## Service Profile

| Property | Current Value |
|----------|---------------|
| Framework | Actix Web 4 |
| Runtime | Tokio |
| Database | PostgreSQL through SQLx |
| Default base URL | `http://localhost:8080` |
| Default bind address | `0.0.0.0:8080` |
| Docker app port | `8000` inside container, mapped to host `8080` by Compose |
| Content type | `application/json` for API requests and responses |
| Request body limit | `256 KiB` for JSON payloads |
| XML schema validation | Embedded UBL schemas through `fastxml` |
| Invoice canonicalization | C14N 1.1 |
| Hash algorithm | SHA-256 |
| PKI | OpenSSL X.509 certificates and RSA signatures |
| Logging | JSON tracing logs, default filter `warn` |

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | No | Built from `POSTGRES_*` vars | Full PostgreSQL URL. Takes precedence over individual database vars. |
| `POSTGRES_USER` | No | `postgres` | Used when `DATABASE_URL` is absent. |
| `POSTGRES_PASSWORD` | No | `password` | Used when `DATABASE_URL` is absent. |
| `POSTGRES_DB` | No | `stc-server` | Used when `DATABASE_URL` is absent. |
| `POSTGRES_HOST` | No | `localhost` | Used when `DATABASE_URL` is absent. |
| `POSTGRES_PORT` | No | `5432` | Used when `DATABASE_URL` is absent. |
| `SEC_PRIVATE_KEY` | Yes | None | Base64-encoded PEM private key used by the server. |
| `SEC_CERTIFICATE` | Yes | None | Base64-encoded PEM server/STC certificate. |
| `PORT` | No | `8080` | HTTP listen port. |
| `RUST_LOG` | No | `warn` | Tracing filter. |
| `RUST_BACKTRACE` | No | Off | Set to `1` for Rust backtraces. |

### Crypto Material

The helper script creates a local root CA and server certificate.

```bash
./scripts/key_generation.sh
```

Use these files for local development:

- `stc-pki/server/server.key`: PEM private key.
- `stc-pki/server/server.crt`: PEM server certificate.
- `stc-pki/base64/server.key.b64`: value for `SEC_PRIVATE_KEY`.
- `stc-pki/base64/server.crt.b64`: value for `SEC_CERTIFICATE`.

The code expects PEM contents after base64 decoding. Enrollment responses return PEM certificate text in JSON, not base64 DER.

## Response Shapes

Most API responses use this generic shape:

```json
{
  "success": true,
  "message": "string",
  "data": {}
}
```

Errors return `success: false` through `ApiResponse<T>` with a sanitized message and stable error code. Detailed implementation errors are logged server-side and are not exposed to clients.

```json
{
  "success": false,
  "message": "Request body must be valid JSON",
  "data": {
    "error": {
      "code": "invalid_json"
    }
  }
}
```

## Endpoints

### GET `/`

Serves the STC home page with navigation to the e-invoicing portal.

Success response:

```http
200 OK
Content-Type: text/html; charset=utf-8
```

### GET `/e-invoicing`

Serves the taxpayer portal for TIN/password sign-in, e-invoicing description, enrollment token generation, persisted production invoice reports, and navigation to the sandbox and API reference.

### GET `/sandbox`

Serves the sandbox console for CSR command generation, device enrollment, canonical invoice payload preparation, and sandbox clearance/reporting requests.

### GET `/api`

Redirects to the Swagger UI at `/api/` for public integration API documentation. The Swagger document includes only `/prod/enrollment/enroll`, `/prod/invoices/clear`, `/sandbox/invoices/clear`, `/prod/invoices/report`, `/sandbox/invoices/report`, and `/health_check`.

### GET `/api/openapi.json`

Serves the generated OpenAPI JSON specification consumed by Swagger UI.

### GET `/health_check`

Health endpoint used by scripts and deployment checks.

Success response:

```http
200 OK
```

The response body is empty.

### POST `/prod/enrollment/enroll`

Enrolls a device by validating an enrollment token, parsing a DER CSR, issuing a certificate, and inserting a `devices` row.

Request body:

```json
{
  "token": "100011:550e8400-e29b-41d4-a716-446655440000",
  "csr": "BASE64_DER_CSR"
}
```

CSR subject requirements:

| Subject Field | OpenSSL NID | Meaning | Example |
|---------------|-------------|---------|---------|
| `serialNumber` | `SERIALNUMBER` | Device UUID | `550e8400-e29b-41d4-a716-446655440000` |
| `O` or `organizationName` | `ORGANIZATIONNAME` | Taxpayer TIN | `100011` |

Success response:

```json
{
  "success": true,
  "message": "enrolled",
  "data": {
    "certificate": "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----\n"
  }
}
```

Error responses include:

```json
{
  "success": false,
  "message": "CSR is invalid",
  "data": {
    "error": {
      "code": "invalid_csr"
    }
  }
}
```

```json
{
  "success": false,
  "message": "Invalid or expired token",
  "data": {
    "error": {
      "code": "invalid_or_expired_token"
    }
  }
}
```

```json
{
  "success": false,
  "message": "Supplier TIN not registered",
  "data": {
    "error": {
      "code": "supplier_tin_not_registered"
    }
  }
}
```

```json
{
  "success": false,
  "message": "Device is already enrolled",
  "data": {
    "error": {
      "code": "device_already_enrolled"
    }
  }
}
```

### POST `/sandbox/invoices/clear`

Submits an invoice for sandbox clearance. The invoice must contain a clearance profile and a valid device certificate already known to the server. Sandbox mode validates the invoice end-to-end, returns a stamped clearance response, but does not persist any data or update the ICV/PIH chain.

Request headers:

```http
Content-Type: application/json
```

Request body:

```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "invoice_hash": "BASE64_SHA256_HASH",
  "invoice": "BASE64_UBL_INVOICE_XML"
}
```

Success response:

```json
{
  "success": true,
  "message": "Invoice cleared",
  "data": {
    "cleared_invoice": "BASE64_CLEARED_INVOICE_XML"
  }
}
```

Invalid parse response:

```json
{
  "success": false,
  "message": "Invalid invoice data",
  "data": {
    "error": {
      "code": "invalid_invoice_data"
    }
  }
}
```

Pipeline failure response:

```json
{
  "success": false,
  "message": "Invoice failed validation",
  "data": {
    "error": {
      "code": "invoice_validation_failed"
    }
  }
}
```

### POST `/prod/invoices/clear`

Submits an invoice for production clearance. Behaves identically to sandbox clearance but persists the cleared invoice and updates the device ICV/PIH chain. Validation failures are recorded in the `rejected_invoices` table.

Request headers:

```http
Content-Type: application/json
```

Request body:

```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "invoice_hash": "BASE64_SHA256_HASH",
  "invoice": "BASE64_UBL_INVOICE_XML"
}
```

Success response:

```json
{
  "success": true,
  "message": "Invoice cleared",
  "data": {
    "cleared_invoice": "BASE64_CLEARED_INVOICE_XML"
  }
}
```

Inactive device response:

```json
{
  "success": false,
  "message": "Device is not enabled",
  "data": {
    "error": {
      "code": "device_inactive"
    }
  }
}
```

Pipeline failure response (recorded as rejected invoice):

```json
{
  "success": false,
  "message": "Invoice failed validation",
  "data": {
    "error": {
      "code": "invoice_validation_failed"
    }
  }
}
```

### POST `/sandbox/invoices/report`

Submits an invoice for sandbox reporting. The invoice must contain a reporting profile and a valid device certificate already known to the server. Sandbox mode validates the invoice end-to-end but does not persist invoices or update the ICV/PIH chain.

Request headers:

```http
Content-Type: application/json
```

Request body:

```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "invoice_hash": "BASE64_SHA256_HASH",
  "invoice": "BASE64_UBL_INVOICE_XML"
}
```

Success response:

```json
{
  "success": true,
  "message": "Invoice reported",
  "data": null
}
```

Invalid invoice data (invalid parse) response:

```json
{
  "success": false,
  "message": "Invalid invoice data",
  "data": {
    "error": {
      "code": "invalid_invoice_data"
    }
  }
}
```

Pipeline failure response:

```json
{
  "success": false,
  "message": "Invoice failed validation",
  "data": {
    "error": {
      "code": "invoice_validation_failed"
    }
  }
}
```

### POST `/prod/invoices/report`

Submits an invoice for production reporting. Behaves identically to sandbox reporting but persists the submitted invoice and updates the device ICV/PIH chain. Validation failures are recorded in the `rejected_invoices` table.

Request headers:

```http
Content-Type: application/json
```

Request body:

```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "invoice_hash": "BASE64_SHA256_HASH",
  "invoice": "BASE64_UBL_INVOICE_XML"
}
```

Success response:

```json
{
  "success": true,
  "message": "Invoice reported",
  "data": null
}
```

Inactive device response:

```json
{
  "success": false,
  "message": "Device is not enabled",
  "data": {
    "error": {
      "code": "device_inactive"
    }
  }
}
```

## Enrollment Flow

### Token Generation

The e-invoicing portal authenticates a taxpayer by TIN and password before generating a device enrollment token. It generates a token in this format:

```text
{tin}:{uuid}
```

The service stores only `SHA256(token)` in `csr_challenges.token_hash`. The token expires after 5 minutes and can be used once. Taxpayer passwords are stored as Argon2 hashes in `taxpayers.password_hash`.

Expired unused tokens are cleaned by a background task every hour.

### CSR Validation And Certificate Issuance

`POST /prod/enrollment/enroll` performs this work:

1. Base64-decodes the CSR.
2. Parses the CSR as DER.
3. Hashes the supplied token and finds an unused, unexpired token row.
4. Verifies the CSR signature using the CSR public key.
5. Signs a new X.509 certificate using the server private key and certificate.
6. Extracts `serialNumber` from the CSR as the device UUID.
7. Extracts `organizationName` from the CSR as the taxpayer TIN.
8. Verifies the TIN exists in `taxpayers`.
9. Inserts a new `devices` row with `current_icv = 0` and initial PIH.
10. Marks the token used.

The generated certificate validity period is 356 days.

## Invoice Submission

### Taxpayer Portal Report

After sign-in, the e-invoicing portal loads a taxpayer invoice report by authenticating the TIN/password again and combining successful `invoices` rows with failed `rejected_invoices` rows. Successful rows are filtered through `invoices.device_id -> devices.device_uuid -> devices.tin`. Failed rows are filtered through `rejected_invoices.device_id` when available, with a fallback to `rejected_invoices.supplier_tin` for parse failures that could still expose the supplier TIN.

The report includes persisted production submissions only. Sandbox submissions use the `/sandbox/invoices/*` path prefix, skip persistence whether successful or failed, and do not appear in the report.

Summary fields cover all persisted taxpayer production submissions and are computed by an aggregate query. The invoice table is capped at the latest 10 metadata rows ordered by `created_at DESC`; it does not load successful `invoice_bytes` or rejected invoice payloads. Returned fields include total submissions, successful count, failed count, clearance/reporting success and failure counts, unique device count, latest submission timestamp, row limit, and rows with UUID, invoice type, device UUID, created timestamp, hash/submitted hash, status, error code, and error message.

### Request DTO

The Rust DTO for clearance and reporting is:

```rust
pub struct SubmitInvoiceDto {
    pub uuid: String,
    pub invoice_hash: String,
    pub invoice: String,
}
```

Parsing performs these operations before the pipeline runs:

1. Base64-decodes `invoice` to XML bytes.
2. Extracts the embedded certificate from XML.
3. Extracts the invoice node and canonicalizes it with C14N 1.1.
4. Base64-decodes `invoice_hash` to raw bytes.
5. Base64-decodes the extracted certificate and parses it as DER X.509.
6. Parses `uuid` as a UUID.
7. Extracts the supplier TIN from the invoice XML.
8. Extracts the device UUID from the certificate subject `serialNumber` and loads the device from the database.

### Profile Selection

The route determines the expected invoice type:

| Route | Expected type |
|-------|---------------|
| `POST /invoices/{profile}/clear` | `clearance` |
| `POST /invoices/{profile}/report` | `reporting` |

The implementation verifies the XML profile against this expected type through `verify_invoice_type`.

### Hash Computation

Invoice hash verification uses SHA-256 over the canonicalized invoice bytes.

Conceptually:

```bash
xmllint --c14n11 invoice.xml | openssl dgst -sha256 -binary | base64 -w 0
```

The exact extracted XML subtree and canonicalization path should match the server implementation in `src/services/xml/`.

### Clearance Output (all `/invoices/*/clear` calls)

Clearance mode returns the cleared invoice as base64 XML. During clearance, the service:

1. Reuses the canonical invoice hash computed during validation.
2. Updates `xades:SigningTime`.
3. Hashes canonicalized signed properties.
4. Updates `SignedInfo` references with the invoice hash and signed-properties hash.
5. Canonicalizes `SignedInfo`.
6. Signs canonicalized `SignedInfo` with the server private key.
7. Replaces the XML signature value.
8. Injects QR data derived from invoice hash and signature.
9. Base64-encodes the final XML.

### Reporting Output (all `/invoices/*/report` calls)

Reporting mode does not stamp or sign the invoice. In production mode (`/prod/invoices/report`), it stores the submitted invoice XML as UTF-8 text in `invoices.invoiceb64`.

### Rejected Production Invoices

For production `/prod/invoices/clear` and `/prod/invoices/report` requests that reach the invoice DTO handler but fail parsing, active-device checks, or invoice processing, the server stores a row in `rejected_invoices` before returning the API error. The stored `error_code`, `error_message`, and `http_status` match the sanitized API error response sent to the client.

If storing the rejected production invoice fails, the server logs the persistence failure and returns `Internal server error` because rejected-invoice persistence is required for production failures.

Invalid JSON, unsupported content type, request-body read errors, and body-size limit failures are not stored because they are rejected before the route receives a `SubmitInvoiceDto`.

## Validation Pipeline

The shared validation pipeline runs stateless invoice checks in this order:

1. UTF-8 conversion of the invoice XML.
2. UBL schema validation.
3. Invoice type/profile validation.
4. SHA-256 invoice hash verification against `invoice_hash`.
5. XAdES-BES signature validation.
6. Certificate validity and CA signature verification using the server certificate.
7. Supplier TIN binding check between invoice XML and certificate `organizationName`.
8. Supplier TIN ownership check against the enrolled device `tin`.
9. Customer TIN existence check for clearance invoices only.
10. Customer TIN must not equal supplier TIN for clearance invoices only.

After shared validation, non-sandbox clearance and reporting both lock the device row, verify ICV and PIH against the locked row, update ICV and PIH, save the invoice, and commit the transaction. Duplicate invoice UUIDs are rejected by the database insert constraint.

## Sandbox Mode

Sandbox mode is intended for validating invoice structure and signatures without mutating chain state.

Skipped in sandbox mode:

- Locked ICV/PIH chain verification.
- Database persistence of the invoice.
- ICV verification and update.
- Device PIH update.

Still performed in sandbox mode:

- Invoice parsing.
- Device lookup from the embedded certificate.
- Active-device check.
- UBL schema validation.
- Invoice type validation.
- Invoice hash verification.
- XAdES-BES validation.
- Certificate verification.
- Supplier certificate/device TIN checks.
- Customer TIN checks for clearance invoices.
- Clearance stamping/signing for all clearance responses.

Sandbox or production mode is determined by the URL path segment:

| Path segment | Behavior |
|--------------|----------|
| `/sandbox/invoices/` | Invoice validated but never persisted; mutations skipped. |
| `/prod/invoices/` | Full validation, persistence, and ICV/PIH chain update. |

No header-based sandbox switching is supported.

## Database Schema

Migrations run automatically on startup with `Migrator::new("./migrations")`.

### `taxpayers`

```sql
CREATE TABLE taxpayers (
    tin VARCHAR(10) PRIMARY KEY,
    name TEXT NOT NULL,
    address TEXT,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

Seed taxpayers:

| TIN | Name | Demo password |
|-----|------|---------------|
| `100011` | `Test Supplier Company` | `password` |
| `100021` | `Test Customer Company` | `password` |

### `devices`

```sql
CREATE TABLE devices (
    device_uuid UUID PRIMARY KEY,
    tin VARCHAR(10) NOT NULL REFERENCES taxpayers(tin),
    current_icv INTEGER NOT NULL DEFAULT 0,
    last_pih BYTEA NOT NULL DEFAULT '\x5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9'::bytea,
    is_active BOOLEAN DEFAULT TRUE,
    onboarded_at TIMESTAMPTZ DEFAULT NOW()
);
```

Initial PIH is SHA-256 of `b"0"`:

```text
5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9
```

### `csr_challenges`

```sql
CREATE TABLE csr_challenges (
    token_hash BYTEA PRIMARY KEY,
    company_id TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '5 minutes'),
    used_at TIMESTAMPTZ
);

CREATE INDEX idx_csr_expiry ON csr_challenges(expires_at) WHERE used_at IS NULL;
CREATE INDEX idx_csr_challenges_company_id ON csr_challenges(company_id);
```

### `invoices`

Current logical schema after migrations:

```sql
CREATE TABLE invoices (
    uuid UUID PRIMARY KEY,
    hash BYTEA NOT NULL CHECK (octet_length(hash) = 32),
    device_id UUID REFERENCES devices(device_uuid),
    invoice_bytes BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    invoice_type TEXT DEFAULT 'reporting' CHECK (invoice_type IN ('reporting', 'clearance'))
);

CREATE UNIQUE INDEX idx_invoices_hash ON invoices(hash);
CREATE INDEX idx_invoices_lookup ON invoices (device_id, invoice_type, created_at DESC);
```

The migrations also add a named unique constraint on `uuid`. Because `uuid` is already the primary key, this is redundant but present in the migration history.

### `rejected_invoices`

```sql
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
```

## Concurrency And State

The service maintains per-device chain state through `devices.current_icv` and `devices.last_pih`.

Non-sandbox invoice persistence uses a database transaction:

1. Begin transaction.
2. Fetch the device row with `FOR UPDATE`.
3. Extract and verify invoice ICV against the locked row.
4. Extract and verify invoice PIH against the locked row.
5. Update device ICV and PIH.
6. Insert invoice row.
7. Commit transaction.

This prevents concurrent submissions for the same device from racing the ICV/PIH update.

## Operational Notes

- The server starts only after it connects to PostgreSQL, runs migrations, loads crypto material, and compiles/loads the XSD schema validator.
- The JSON request limit is `256 KiB`; larger invoices will be rejected by Actix before route logic runs.
- There is no debug endpoint for raw invoice rows; production invoice data is only exposed through the authenticated taxpayer portal report.
- Error responses expose sanitized messages and stable error codes; detailed implementation errors remain in server logs.
- The e-invoicing portal keeps the taxpayer password only in browser memory for the current session and resends it when generating an enrollment token.
- The repository integration shell scripts are useful development helpers but are not the source of truth for endpoint contracts.

## Examples

### Generate Local Keys

```bash
./scripts/key_generation.sh
export SEC_PRIVATE_KEY="$(tr -d '\n' < stc-pki/base64/server.key.b64)"
export SEC_CERTIFICATE="$(tr -d '\n' < stc-pki/base64/server.crt.b64)"
```

### Start Locally

```bash
export DATABASE_URL=postgres://postgres:password@localhost:5432/stc-server
cargo run
```

### Health Check

```bash
curl -i http://localhost:8080/health_check
```

### Generate Enrollment Token

Open `http://localhost:8080/e-invoicing`, sign in with TIN `100011` and password `password`, then click `Generate enrollment token`.

### Generate CSR

```bash
openssl genrsa -out device.key 2048
openssl req -new \
  -key device.key \
  -outform DER \
  -subj "/O=100011/serialNumber=550e8400-e29b-41d4-a716-446655440000" \
  | base64 -w 0 > device.csr.b64
```

### Enroll Device

```bash
TOKEN="100011:replace-with-token-uuid"
CSR="$(tr -d '\n' < device.csr.b64)"

curl -X POST http://localhost:8080/prod/enrollment/enroll \
  -H "Content-Type: application/json" \
  -d "{\"token\":\"$TOKEN\",\"csr\":\"$CSR\"}"
```

### Submit Clearance Invoice (Sandbox)

```bash
curl -X POST http://localhost:8080/sandbox/invoices/clear \
  -H "Content-Type: application/json" \
  -d '{
    "uuid": "550e8400-e29b-41d4-a716-446655440000",
    "invoice_hash": "BASE64_SHA256_HASH",
    "invoice": "BASE64_UBL_INVOICE_XML"
  }'
```

### Submit Clearance Invoice (Production)

```bash
curl -X POST http://localhost:8080/prod/invoices/clear \
  -H "Content-Type: application/json" \
  -d '{
    "uuid": "550e8400-e29b-41d4-a716-446655440000",
    "invoice_hash": "BASE64_SHA256_HASH",
    "invoice": "BASE64_UBL_INVOICE_XML"
  }'
```

### Submit Reporting Invoice (Sandbox)

```bash
curl -X POST http://localhost:8080/sandbox/invoices/report \
  -H "Content-Type: application/json" \
  -d '{
    "uuid": "550e8400-e29b-41d4-a716-446655440000",
    "invoice_hash": "BASE64_SHA256_HASH",
    "invoice": "BASE64_UBL_INVOICE_XML"
  }'
```

### Submit Reporting Invoice (Production)

```bash
curl -X POST http://localhost:8080/prod/invoices/report \
  -H "Content-Type: application/json" \
  -d '{
    "uuid": "550e8400-e29b-41d4-a716-446655440000",
    "invoice_hash": "BASE64_SHA256_HASH",
    "invoice": "BASE64_UBL_INVOICE_XML"
  }'
```
