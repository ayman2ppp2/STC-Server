# STC-Server

Sudanese Taxation Chamber (STC) Electronic Invoicing Backend

A high-performance, secure backend for the national electronic invoicing system, implementing UBL 2.1 invoice validation, XAdES digital signatures, and dual-mode invoice processing (clearance and reporting).

## Features

- **UBL 2.1 Invoice Validation** - XSD schema validation for Universal Business Language invoices
- **XAdES Digital Signatures** - XML Advanced Electronic Signatures for non-repudiation
- **PKI-based Device Enrollment** - CSR-based certificate issuance for taxpayers
- **Dual-Mode Processing** - Real-time clearance and deferred reporting
- **PostgreSQL** - ACID-compliant persistent storage
- **Rust** - Memory safety, zero-cost abstractions, high concurrency

## Table of Contents

- [Quick Start](#quick-start)
- [Environment Variables](#environment-variables)
- [Database Setup](#database-setup)
- [API Endpoints](#api-endpoints)
- [Invoice Processing](#invoice-processing)
- [Device Enrollment](#device-enrollment)
- [Sandbox Mode](#sandbox-mode)
- [Database Schema](#database-schema)
- [Testing](#testing)
- [Architecture](#architecture)

## Quick Start

### Prerequisites

- Rust 1.70+ (with cargo)
- PostgreSQL 15+
- OpenSSL development headers
- Libxml development headers

### 1. Clone and Configure

```bash
git clone https://github.com/yourrepo/STC-Server.git
cd STC-Server
```

### 2. Environment Variables

Create a `.env` file:

```bash
# Database
DATABASE_URL=postgresql://postgres:password@localhost:5432/stc_server

# Cryptography (generate with scripts/key_generation.sh)
SEC_PRIVATE_KEY=<base64_encoded_private_key>
SEC_CERTIFICATE=<base64_encoded_certificate>

# Optional
PORT=8080
RUST_BACKTRACE=1
```

### 3. Generate Crypto Keys

```bash
# Generate CA certificate and server keys
./scripts/key_generation.sh
```

This creates:
- `certs/ca.key` - CA private key
- `certs/ca.crt` - CA certificate
- `certs/server.key` - Server private key
- `certs/server.crt` - Server certificate

Set environment variables:
```bash
export SEC_PRIVATE_KEY=$(base64 -w0 certs/server.key)
export SEC_CERTIFICATE=$(base64 -w0 certs/server.crt)
```

### 4. Database Setup

```bash
# Create database
createdb stc_server

# Run migrations (automatic on startup)
cargo run

# Or run migrations manually
cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx migrate run
```

### 5. Run the Server

```bash
cargo run
```

Server starts on `http://localhost:8080`

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | - | PostgreSQL connection string |
| `SEC_PRIVATE_KEY` | Yes | - | Base64-encoded server private key (PEM) |
| `SEC_CERTIFICATE` | Yes | - | Base64-encoded server CA certificate (PEM) |
| `PORT` | No | 8080 | Server port |
| `RUST_BACKTRACE` | No | 0 | Enable Rust backtraces (0 or 1) |

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | Health check |
| `/health_check` | GET | Server health status |
| `/onboard` | GET | Serve onboarding HTML form |
| `/onboard` | POST | Generate enrollment token |
| `/enroll` | POST | Enroll device with CSR |
| `/clear` | POST | Submit invoice for clearance |
| `/reporting` | POST | Submit invoice for reporting |
| `/get_invoices` | GET | Get invoice count (debug) |
| `/verify_qr` | POST | Verify QR code signature |

## Invoice Processing

### Clearance Mode (`POST /clear`)

Real-time invoice validation and signing:

```
1. Receive invoice submission
2. Validate:
   - UUID uniqueness
   - XSD schema (UBL 2.1)
   - Invoice type (profile ID)
   - Hash (SHA256)
   - PIH chain (previous invoice hash)
   - Certificate (CA chain)
   - Signature
   - Supplier/Customer TINs
   - ICV
3. Stamp invoice:
   - Update signing time
   - Update hashes in SignedInfo
   - Sign with server key
   - Generate QR code
4. Save to database
5. Return stamped invoice
```

### Reporting Mode (`POST /report`)

Deferred invoice submission:

```
1. Receive invoice submission
2. Validate (same as clearance)
3. No stamping - hash computed locally
4. Save to database
5. Return success acknowledgment
```

### Submit Invoice Request

```json
{
  "invoice_hash": "base64_encoded_sha256_hash",
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "invoice": "base64_encoded_ubl_invoice_xml"
}
```

### Response (Clearance)

```json
{
  "success": true,
  "message": "Invoice cleared successfully",
  "data": {
    "invoice": "base64_encoded_stamped_invoice"
  }
}
```

## Device Enrollment

### Flow

```
1. Generate token: POST /onboard
   └── Returns: { token, message }

2. Create device: POST /enroll
   └── Body: { token, csr }
   └── Returns: { certificate }
```

### CSR Requirements

The CSR must include:

| Field | Description | Example |
|-------|-------------|---------|
| `serialNumber` | Device UUID | `550e8400-e29b-41d4-a716-446655440000` |
| `organizationName` | Taxpayer TIN | `399999999900003` |

### Generate Test CSR

```bash
# Generate key
openssl genrsa -out device.key 2048

# Generate CSR
openssl req -new \
  -key device.key \
  -out device.csr \
  -subj "/O=399999999900003/serialNumber=550e8400-e29b-41d4-a716-446655440000"

# Convert to DER base64
openssl req -in device.csr -outform DER | base64 -w0
```

## Sandbox Mode

Use the `X-Sandbox-Mode: true` header to skip validation:

```
POST /clear
X-Sandbox-Mode: true
```

**Skipped validations in sandbox:**
- UUID uniqueness check
- PIH chain verification
- Database persistence (invoice not saved)
- ICV increment

**Still validated:**
- XSD schema
- Hash computation
- Signature verification
- Certificate validation
- TIN verification

Use sandbox for testing invoice format and signatures without affecting the production chain.

## Database Schema

### taxpayers

Taxpayer registry (TIN lookup).

```sql
CREATE TABLE taxpayers (
    tin VARCHAR(10) PRIMARY KEY,
    name TEXT NOT NULL,
    address TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### devices

Registered devices with ICV/PIH chain state.

```sql
CREATE TABLE devices (
    device_uuid UUID PRIMARY KEY,
    tin VARCHAR(10) REFERENCES taxpayers(tin),
    current_icv INTEGER NOT NULL DEFAULT 0,
    last_pih BYTEA NOT NULL DEFAULT 'sha256(b"0")',
    is_active BOOLEAN DEFAULT TRUE,
    onboarded_at TIMESTAMPTZ DEFAULT NOW()
);
```

**Initial PIH**: `5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9` (SHA-256 of `b"0"`)

### invoices

Stored invoices with hash chain.

```sql
CREATE TABLE invoices (
    uuid UUID PRIMARY KEY,
    hash BYTEA NOT NULL,
    invoiceb64 TEXT,
    device_id UUID REFERENCES devices(device_uuid),
    invoice_type TEXT CHECK (invoice_type IN ('reporting', 'clearance')),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_invoices_lookup ON invoices (device_id, invoice_type, created_at DESC);
```

### csr_challenges

Enrollment tokens for device registration.

```sql
CREATE TABLE csr_challenges (
    token_hash BYTEA PRIMARY KEY,
    company_id TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '5 minutes'),
    used_at TIMESTAMPTZ
);

CREATE INDEX idx_csr_challenges_company_id ON csr_challenges(company_id);
```

## Testing

### Run Integration Tests

```bash
# All tests
./scripts/tests/run_all.sh

# Individual tests
./scripts/tests/test_clearance.sh
./scripts/tests/run_all.sh
./scripts/tests/test_reporting.sh
./scripts/tests/test_enrollment.sh
```

### Set Base URL

```bash
export BASE_URL=http://localhost:8080
./scripts/tests/run_all.sh
```

### Manual Testing

```bash
# Health check
curl http://localhost:8080/health_check

# Generate token
curl -X POST http://localhost:8080/onboard \
  -H "Content-Type: application/json" \
  -d '{"name":"Company","email":"test@example.com","company_id":"399999999900003"}'

# Submit invoice (sandbox)
curl -X POST http://localhost:8080/clear \
  -H "Content-Type: application/json" \
  -H "X-Sandbox-Mode: true" \
  -d '{"invoice_hash":"...","uuid":"...","invoice":"..."}'
```

## Architecture

```
src/
├── main.rs                 # Entry point, route definitions
├── config/                # Configuration
│   ├── crypto_config.rs   # Private key, certificate
│   ├── db_config.rs       # PostgreSQL connection
│   └── xsd_config.rs      # UBL schema validator
├── models/                # Data structures
│   ├── device.rs          # Device entity
│   ├── enrollment_dto.rs  # Enrollment request/response
│   └── submit_invoice_dto.rs  # Invoice submission
├── routes/                # HTTP handlers
│   ├── enroll.rs          # Device enrollment
│   ├── invoice_controller.rs  # Clearance/Reporting
│   └── token_generator.rs    # Token creation
└── services/             # Business logic
    ├── validation_service.rs  # Invoice validation pipeline
    ├── clearance_service.rs  # Clearance processing
    ├── reporting_service.rs  # Reporting processing
    ├── pki_service.rs        # Certificate operations
    ├── icv_service.rs        # Invoice counter
    ├── pih_service.rs        # Hash chain
    ├── tin_service.rs        # TIN verification
    └── extractors.rs         # XML parsing
```

### Validation Pipeline

```
1. UUID Uniqueness      → check_uuid()
2. Schema Validation    → validate_schema() (UBL 2.1)
3. Invoice Type        → verify_invoice_type()
4. Hash Verification   → compute_hash() == received_hash
5. PIH Chain          → verify_pih() (skipped in sandbox)
6. Certificate        → verify_cert_with_ca()
7. Signature          → verify_signature_with_cert()
8. Supplier TIN       → verify_supplier_tin()
9. Customer TIN       → verify_customer_tin() (clearance only)
```

### Concurrency Control

- `SELECT ... FOR UPDATE` on device row during ICV/PIH update
- Database transaction for atomic invoice save
- Background task for expired token cleanup (hourly)

## License

This is a prototype for demonstration purposes. Not intended for production use without security audit.
