# STC Server

Rust backend for the Sudanese Taxation Chamber electronic invoicing prototype. The service validates UBL 2.1 invoices, verifies XAdES-BES signatures, enrolls taxpayer devices through a CSR-based PKI flow, and processes invoices through clearance or reporting paths.

This repository is a prototype. Do not use it in production without a security review, operational hardening, and a compliance review against the final STC e-invoicing specification.

## Contents

- [What It Does](#what-it-does)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Configuration](#configuration)
- [Run Locally](#run-locally)
- [Run With Docker Compose](#run-with-docker-compose)
- [API Overview](#api-overview)
- [Device Enrollment](#device-enrollment)
- [Invoice Processing](#invoice-processing)
- [Database](#database)
- [Testing](#testing)
- [Technical Documentation](#technical-documentation)

## What It Does

- Validates invoice XML against embedded UBL 2.1 schemas.
- Canonicalizes invoice XML with C14N 1.1 and verifies SHA-256 invoice hashes.
- Validates XAdES-BES signature structure, references, certificate binding, and signature value.
- Enrolls taxpayer devices by accepting a DER CSR and issuing an STC-signed certificate.
- Maintains per-device invoice chain state with ICV and PIH values.
- Supports clearance mode, where the server stamps/signs the invoice and injects QR data.
- Supports reporting mode, where the server validates and stores the submitted invoice without stamping it.
- Stores taxpayers, devices, enrollment challenges, and invoices in PostgreSQL.
- Emits JSON tracing logs through `tracing` and `tracing-actix-web`.

## Architecture

```text
src/
|-- main.rs                         # Actix server setup, routes, migrations, tracing
|-- config/                         # PostgreSQL, crypto, and XSD schema configuration
|-- models/                         # Request/response DTOs and database models
|-- routes/                         # HTTP handlers
|-- services/
|   |-- crypto/                     # PKI, XAdES, QR verification
|   |-- db/                         # Database reads/writes and chain state
|   |-- pipeline/                   # Enrollment, onboarding, validation, clearance, reporting
|   `-- xml/                        # XML extraction, canonicalization, validation, editing
|-- static/                         # Onboarding HTML
`-- xsd/                            # Embedded UBL schemas
```

The main request paths are:

- `POST /onboard` creates a short-lived enrollment token for a registered taxpayer.
- `POST /enroll` validates the token and CSR, issues a device certificate, and creates a device row.
- `POST /clear` validates, stamps, signs, stores, and returns a cleared invoice.
- `POST /report` validates and stores a reported invoice without server stamping.

## Prerequisites

- Rust toolchain with edition 2024 support.
- PostgreSQL.
- OpenSSL CLI and development libraries.
- libxml2 runtime/development libraries.
- `sqlx-cli` if you want to run migrations manually.
- Docker and Docker Compose if using the containerized setup.

On Debian/Ubuntu, typical native dependencies are:

```bash
sudo apt-get install -y pkg-config libssl-dev libxml2-dev clang libclang-dev postgresql-client
```

## Configuration

The server reads configuration from environment variables.

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | No | Built from `POSTGRES_*` vars | PostgreSQL connection URL. |
| `POSTGRES_USER` | No | `postgres` | Used only when `DATABASE_URL` is not set. |
| `POSTGRES_PASSWORD` | No | `password` | Used only when `DATABASE_URL` is not set. |
| `POSTGRES_DB` | No | `stc-server` | Used only when `DATABASE_URL` is not set. |
| `POSTGRES_HOST` | No | `localhost` | Used only when `DATABASE_URL` is not set. |
| `POSTGRES_PORT` | No | `5432` | Used only when `DATABASE_URL` is not set. |
| `SEC_PRIVATE_KEY` | Yes | None | Base64-encoded PEM private key used to sign certificates and cleared invoices. |
| `SEC_CERTIFICATE` | Yes | None | Base64-encoded PEM STC certificate used as the issuing/verification certificate. |
| `PORT` | No | `8080` | HTTP listen port. |
| `RUST_LOG` | No | `warn` | Tracing filter, for example `info` or `stc_server=debug`. |
| `RUST_BACKTRACE` | No | Off | Enables Rust backtraces when set to `1`. |

The JSON request body limit is currently `256 KiB`.

## Run Locally

1. Generate local PKI material.

```bash
./scripts/key_generation.sh
```

This creates files under `stc-pki/`, including:

- `stc-pki/root/stc_root_ca.key`
- `stc-pki/root/stc_root_ca.crt`
- `stc-pki/server/server.key`
- `stc-pki/server/server.crt`
- `stc-pki/base64/server.key.b64`
- `stc-pki/base64/server.crt.b64`
- `stc-pki/base64/stc_root_ca.crt.b64`

2. Export the required crypto environment variables.

```bash
export SEC_PRIVATE_KEY="$(tr -d '\n' < stc-pki/base64/server.key.b64)"
export SEC_CERTIFICATE="$(tr -d '\n' < stc-pki/base64/server.crt.b64)"
```

3. Start PostgreSQL and run migrations.

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
./scripts/init_db.sh
```

If PostgreSQL is already running, set `SKIP_DOCKER=1` before running `scripts/init_db.sh`.

4. Run the server.

```bash
cargo run
```

The server listens on `http://localhost:8080` by default.

## Run With Docker Compose

Generate the PKI material first, then provide the base64 key and certificate to Compose.

```bash
./scripts/key_generation.sh
export SEC_PRIVATE_KEY="$(tr -d '\n' < stc-pki/base64/server.key.b64)"
export SEC_CERTIFICATE="$(tr -d '\n' < stc-pki/base64/server.crt.b64)"
docker compose up --build
```

The Compose setup exposes the app on `http://localhost:8080` and PostgreSQL on `localhost:5432`.

## API Overview

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/` | Plain text hello response. |
| `GET` | `/health_check` | Empty `200 OK` health response. |
| `GET` | `/onboard` | HTML onboarding form. |
| `POST` | `/onboard` | Generate a 5-minute enrollment token for a registered taxpayer. |
| `POST` | `/enroll` | Enroll a device using a token and DER CSR. |
| `POST` | `/clear` | Submit an invoice for clearance. |
| `POST` | `/report` | Submit an invoice for reporting. |
| `GET` | `/get_invoices` | Debug endpoint returning stored invoice count. |
| `POST` | `/verify_qr` | Verify QR payload signature. |

See `TECHNICAL_DOCUMENTATION.md` for exact request and response schemas.

## Device Enrollment

Enrollment is a two-step flow.

1. Generate a token for a taxpayer that exists in the `taxpayers` table.

```bash
curl -X POST http://localhost:8080/onboard \
  -H "Content-Type: application/json" \
  -d '{"name":"Test Company","email":"test@example.com","company_id":"100011"}'
```

2. Generate a CSR whose subject includes the taxpayer TIN and device UUID.

```bash
openssl genrsa -out device.key 2048
openssl req -new \
  -key device.key \
  -outform DER \
  -subj "/O=100011/serialNumber=550e8400-e29b-41d4-a716-446655440000" \
  | base64 -w 0 > device.csr.b64
```

3. Enroll the device with the returned token and base64 DER CSR.

```bash
curl -X POST http://localhost:8080/enroll \
  -H "Content-Type: application/json" \
  -d "{\"token\":\"TOKEN_FROM_ONBOARD\",\"csr\":\"$(tr -d '\n' < device.csr.b64)\"}"
```

The issued certificate is returned as PEM text in `data.certificate`.

## Invoice Processing

Invoice submissions use this JSON shape:

```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "invoice_hash": "BASE64_SHA256_HASH_OF_CANONICAL_INVOICE",
  "invoice": "BASE64_UBL_INVOICE_XML"
}
```

Clearance mode uses `POST /clear` and expects a clearance invoice profile. The server validates the invoice, updates signing metadata, signs the invoice, inserts QR data, stores the cleared invoice, and returns the base64 cleared invoice.

Reporting mode uses `POST /report` and expects a reporting invoice profile. The server validates the invoice, stores the submitted invoice, and returns an acknowledgement without stamping/signing it.

Sandbox mode is controlled by the `X-Sandbox-Mode` header. In sandbox mode, locked ICV/PIH checks, database persistence, and chain-state updates are skipped. Schema, hash, XAdES-BES, certificate, and TIN validation still run.

## Database

Migrations run automatically on startup from `./migrations`. The active logical tables are:

- `taxpayers`: registered taxpayer TINs.
- `devices`: enrolled device UUIDs, taxpayer ownership, current ICV, and last PIH.
- `csr_challenges`: hashed enrollment tokens with expiry and usage state.
- `invoices`: submitted invoice UUIDs, hashes, stored invoice payloads, device IDs, and invoice type.

The seed migration inserts test taxpayers `100011` and `100021`.

## Testing

Run the Rust test/build checks with Cargo:

```bash
cargo check
cargo test
```

The repository also contains shell-based integration scripts under `scripts/tests/`. Some scripts may lag behind route changes, so treat them as development helpers rather than authoritative API documentation.

## Technical Documentation

Use `TECHNICAL_DOCUMENTATION.md` for detailed endpoint contracts, validation order, database schema, and operational notes.

## License And Status

This project is a prototype for demonstration and development. It is not production-ready without further security, compliance, reliability, and observability work.
