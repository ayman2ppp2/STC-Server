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

`POST /onboard` is an exception. It returns a plain onboarding response:

```json
{
  "message": "Token generated successfully. Use this token within 5 minutes.",
  "token": "100011:550e8400-e29b-41d4-a716-446655440000"
}
```

Errors usually return `success: false` through `ApiResponse<T>`. All pipeline errors generally return `data: null`.

## Endpoints

### GET `/`

Returns a plain text greeting.

Success response:

```text
Hello from STC Actix server!
```

### GET `/health_check`

Health endpoint used by scripts and deployment checks.

Success response:

```http
200 OK
```

The response body is empty.

### GET `/onboard`

Serves the static HTML onboarding form from `src/static/token_form.html`.

Success response:

```http
200 OK
Content-Type: text/html
```

### POST `/onboard`

Generates a 5-minute enrollment token for a taxpayer TIN that exists in `taxpayers`.

Request body:

```json
{
  "name": "Test Company",
  "email": "test@example.com",
  "company_id": "100011"
}
```

Current implementation only uses `company_id`. `name` and `email` are accepted by the DTO but are not persisted or validated by the route.

Success response:

```json
{
  "message": "Token generated successfully. Use this token within 5 minutes.",
  "token": "100011:550e8400-e29b-41d4-a716-446655440000"
}
```

Invalid TIN response:

```http
400 Bad Request
```

```json
{
  "success": false,
  "message": "Invalid company ID",
  "data": null
}
```

### POST `/enroll`

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
  "message": "CSR Parsing Error",
  "data": null
}
```

```json
{
  "success": false,
  "message": "Invalid or expired token",
  "data": null
}
```

```json
{
  "success": false,
  "message": "Supplier TIN not registered",
  "data": null
}
```

```json
{
  "success": false,
  "message": "Enrollment failed",
  "data": null
}
```

### POST `/clear`

Submits an invoice for clearance. The invoice must contain a clearance profile and a valid device certificate already known to the server.

Request headers:

```http
Content-Type: application/json
X-Sandbox-Mode: true
```

`X-Sandbox-Mode` is optional. For `/clear`, sandbox is enabled only when the header value equals `true` case-insensitively.

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
  "data": null
}
```

Inactive device response:

```json
{
  "success": false,
  "message": "Device is not enabled",
  "data": null
}
```

Pipeline failure response:

```json
{
  "success": false,
  "message": "Clearance failed",
  "data": {
    "error": "error details"
  }
}
```

### POST `/report`

Submits an invoice for reporting. The invoice must contain a reporting profile and a valid device certificate already known to the server.

Request headers:

```http
Content-Type: application/json
X-Sandbox-Mode: true
```

`X-Sandbox-Mode` is optional. For `/report`, sandbox is enabled when the header is present, regardless of its value.

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

Invalid parse response:

```json
{
  "success": false,
  "message": "Invalid invoice data",
  "data": null
}
```

Pipeline failure response:

```json
{
  "success": false,
  "message": "Reporting failed",
  "data": null
}
```

### GET `/get_invoices`

Debug endpoint that returns the number of rows in `invoices`.

Success response:

```json
42
```

Failure response:

```http
500 Internal Server Error
```

```text
Failed to fetch invoices
```

### POST `/verify_qr`

Verifies the signature embedded in a QR payload.

Request body:

```json
{
  "qr_b64": "BASE64_QR_PAYLOAD"
}
```

Success response:

```json
{
  "success": true,
  "message": "verfied",
  "data": null
}
```

The success message currently contains the typo `verfied`.

Failure response:

```json
{
  "success": false,
  "message": "QR verification failed",
  "data": null
}
```

## Enrollment Flow

### Token Generation

`POST /onboard` checks that `company_id` exists in `taxpayers`. It generates a token in this format:

```text
{tin}:{uuid}
```

The service stores only `SHA256(token)` in `csr_challenges.token_hash`. The token expires after 5 minutes and can be used once.

Expired unused tokens are cleaned by a background task every hour.

### CSR Validation And Certificate Issuance

`POST /enroll` performs this work:

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
2. Extracts the invoice signature and embedded certificate from XML.
3. Extracts the invoice node and canonicalizes it with C14N 1.1.
4. Base64-decodes `invoice_hash` to raw bytes.
5. Base64-decodes the extracted signature.
6. Base64-decodes the extracted certificate and parses it as DER X.509.
7. Parses `uuid` as a UUID.
8. Extracts the supplier TIN from the invoice XML.
9. Extracts the device UUID from the certificate subject `serialNumber` and loads the device from the database.

### Profile Selection

The route determines the expected invoice type:

| Route | Expected type |
|-------|---------------|
| `POST /clear` | `clearance` |
| `POST /report` | `reporting` |

The implementation verifies the XML profile against this expected type through `verify_invoice_type`.

### Hash Computation

Invoice hash verification uses SHA-256 over the canonicalized invoice bytes.

Conceptually:

```bash
xmllint --c14n11 invoice.xml | openssl dgst -sha256 -binary | base64 -w 0
```

The exact extracted XML subtree and canonicalization path should match the server implementation in `src/services/xml/`.

### Clearance Output

Clearance mode returns the cleared invoice as base64 XML. During clearance, the service:

1. Computes the canonical invoice hash.
2. Updates `xades:SigningTime`.
3. Hashes canonicalized signed properties.
4. Updates `SignedInfo` references with the invoice hash and signed-properties hash.
5. Canonicalizes `SignedInfo`.
6. Signs canonicalized `SignedInfo` with the server private key.
7. Replaces the XML signature value.
8. Injects QR data derived from invoice hash and signature.
9. Base64-encodes the final XML.

### Reporting Output

Reporting mode does not stamp or sign the invoice. In non-sandbox mode, it stores the submitted invoice XML as UTF-8 text in `invoices.invoiceb64`.

## Validation Pipeline

The shared validation pipeline runs in this order:

1. UUID uniqueness check against `invoices`, skipped in sandbox mode.
2. UTF-8 conversion of the invoice XML.
3. UBL schema validation.
4. Invoice type/profile validation.
5. SHA-256 invoice hash verification against `invoice_hash`.
6. PIH chain verification, skipped in sandbox mode.
7. XAdES-BES signature validation.
8. Certificate validity and CA signature verification using the server certificate.
9. Supplier TIN binding check between invoice XML and certificate `organizationName`.
10. Supplier TIN existence check against `taxpayers`.
11. Customer TIN existence check for clearance invoices only.
12. Customer TIN must not equal supplier TIN for clearance invoices only.

After shared validation, non-sandbox clearance and reporting both lock the device row, verify ICV, update ICV and PIH, save the invoice, and commit the transaction.

## Sandbox Mode

Sandbox mode is intended for validating invoice structure and signatures without mutating chain state.

Skipped in sandbox mode:

- UUID uniqueness check.
- PIH chain verification.
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
- Supplier TIN checks.
- Customer TIN checks for clearance invoices.
- Clearance stamping/signing for `/clear` responses.

Header behavior differs by route:

| Route | Sandbox condition |
|-------|-------------------|
| `/clear` | Header `X-Sandbox-Mode` exists and value equals `true` case-insensitively. |
| `/report` | Header `X-Sandbox-Mode` exists with any value. |

## Database Schema

Migrations run automatically on startup with `sqlx::migrate!("./migrations")`.

### `taxpayers`

```sql
CREATE TABLE taxpayers (
    tin VARCHAR(10) PRIMARY KEY,
    name TEXT NOT NULL,
    address TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

Seed taxpayers:

| TIN | Name |
|-----|------|
| `100011` | `Test Supplier Company` |
| `100021` | `Test Customer Company` |

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
    invoiceb64 TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    invoice_type TEXT DEFAULT 'reporting' CHECK (invoice_type IN ('reporting', 'clearance'))
);

CREATE UNIQUE INDEX idx_invoices_hash ON invoices(hash);
CREATE INDEX idx_invoices_lookup ON invoices (device_id, invoice_type, created_at DESC);
```

The migrations also add a named unique constraint on `uuid`. Because `uuid` is already the primary key, this is redundant but present in the migration history.

## Concurrency And State

The service maintains per-device chain state through `devices.current_icv` and `devices.last_pih`.

Non-sandbox invoice persistence uses a database transaction:

1. Begin transaction.
2. Fetch the device row with `FOR UPDATE`.
3. Extract and verify invoice ICV against the locked row.
4. Update device ICV and PIH.
5. Insert invoice row.
6. Commit transaction.

This prevents concurrent submissions for the same device from racing the ICV/PIH update.

## Operational Notes

- The server starts only after it connects to PostgreSQL, runs migrations, loads crypto material, and compiles/loads the XSD schema validator.
- The JSON request limit is `256 KiB`; larger invoices will be rejected by Actix before route logic runs.
- `GET /get_invoices` is a debug helper and should not be exposed in a production deployment.
- `POST /clear` currently exposes internal error text in `data.error`; harden this before production.
- `POST /verify_qr` currently returns the success message `verfied`.
- The onboarding HTML includes fields beyond what the JSON route currently uses.
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

```bash
curl -X POST http://localhost:8080/onboard \
  -H "Content-Type: application/json" \
  -d '{"name":"Test Company","email":"test@example.com","company_id":"100011"}'
```

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

curl -X POST http://localhost:8080/enroll \
  -H "Content-Type: application/json" \
  -d "{\"token\":\"$TOKEN\",\"csr\":\"$CSR\"}"
```

### Submit Clearance Invoice

```bash
curl -X POST http://localhost:8080/clear \
  -H "Content-Type: application/json" \
  -H "X-Sandbox-Mode: true" \
  -d '{
    "uuid": "550e8400-e29b-41d4-a716-446655440000",
    "invoice_hash": "BASE64_SHA256_HASH",
    "invoice": "BASE64_UBL_INVOICE_XML"
  }'
```

### Submit Reporting Invoice

```bash
curl -X POST http://localhost:8080/report \
  -H "Content-Type: application/json" \
  -H "X-Sandbox-Mode: true" \
  -d '{
    "uuid": "550e8400-e29b-41d4-a716-446655440000",
    "invoice_hash": "BASE64_SHA256_HASH",
    "invoice": "BASE64_UBL_INVOICE_XML"
  }'
```

### Verify QR Payload

```bash
curl -X POST http://localhost:8080/verify_qr \
  -H "Content-Type: application/json" \
  -d '{"qr_b64":"BASE64_QR_PAYLOAD"}'
```
