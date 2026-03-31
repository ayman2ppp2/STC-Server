# STC Server - Technical Documentation

A comprehensive guide for connecting to and building clients for the Sudanese Taxation Chamber (STC) Electronic Invoicing Backend.

## Table of Contents

1. [Overview](#overview)
2. [Server Information](#server-information)
3. [Authentication Flow](#authentication-flow)
4. [API Endpoints](#api-endpoints)
5. [Data Models](#data-models)
6. [Invoice Submission](#invoice-submission)
7. [Validation Pipeline](#validation-pipeline)
8. [Error Handling](#error-handling)
9. [Code Examples](#code-examples)
10. [Testing](#testing)

---

## Overview

The STC Server is a Rust-based electronic invoicing backend implementing:
- **UBL 2.1** invoice validation
- **XAdES** digital signatures
- **PKI-based** device enrollment
- **Dual-mode** invoice processing (clearance and reporting)

---

## Server Information

| Property | Value |
|----------|-------|
| Protocol | HTTP/HTTPS |
| Default Port | 8080 |
| Content-Type | `application/json` |
| Character Encoding | UTF-8 |

### Base URL
```
http://localhost:8080
```

### Environment Variables (Server)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | - | PostgreSQL connection string |
| `SEC_PRIVATE_KEY` | Yes | - | Base64-encoded server private key (PEM) |
| `SEC_CERTIFICATE` | Yes | - | Base64-encoded server CA certificate (PEM) |
| `PORT` | No | 8080 | Server port |
| `RUST_BACKTRACE` | No | 0 | Enable Rust backtraces (0 or 1) |

---

## Authentication Flow

The STC Server uses a **two-step enrollment process** to authenticate devices:

### Step 1: Generate Enrollment Token

```
Client: POST /onboard
  { "name": "Company", "email": "email@example.com", "company_id": "TIN" }

Server: 200 OK
  { "message": "Token generated...", "token": "TIN:UUID" }
```

**Requirements:**
- `company_id` must be a valid TIN registered in the `taxpayers` table
- Token expires after **5 minutes**

### Step 2: Enroll Device with CSR

```
Client: POST /enroll
  { "token": "TIN:UUID", "csr": "BASE64_DER_CSR" }

Server: 200 OK
  { "success": true, "message": "enrolled", "data": { "certificate": "BASE64_DER_CERT" } }
```

**CSR Requirements:**
| Field | NID | Example |
|-------|-----|---------|
| `serialNumber` | NID_SERIALNUMBER | Device UUID |
| `organizationName` | NID_ORGANIZATIONNAME | Taxpayer TIN |

### Generate Test CSR

```bash
# Generate device key
openssl genrsa -out device.key 2048

# Generate CSR
openssl req -new \
  -key device.key \
  -out device.csr \
  -subj "/O=399999999900003/serialNumber=550e8400-e29b-41d4-a716-446655440000"

# Convert to DER base64
openssl req -in device.csr -outform DER | base64 -w0
```

---

## API Endpoints

### 1. Health Check

```
GET /health_check
```

**Response:**
```json
{ "status": "healthy" }
```

---

### 2. Generate Enrollment Token

```
GET /onboard
```

Serves an HTML form for manual token generation.

---

### 3. Generate Token (Programmatic)

```
POST /onboard
Content-Type: application/json
```

**Request:**
```json
{
  "name": "Company Name",
  "email": "email@example.com",
  "company_id": "399999999900003"
}
```

**Response (Success):**
```json
{
  "success": true,
  "message": "Token generated successfully. Use this token within 5 minutes.",
  "data": {
    "message": "Token generated successfully...",
    "token": "399999999900003:550e8400-e29b-41d4-a716-446655440000"
  }
}
```

**Response (Error - Invalid TIN):**
```json
{
  "success": false,
  "message": "Invalid company ID",
  "data": { "details": "Company ID not found in taxpayer registry" }
}
```

---

### 4. Enroll Device

```
POST /enroll
Content-Type: application/json
```

**Request:**
```json
{
  "token": "399999999900003:550e8400-e29b-41d4-a716-446655440000",
  "csr": "BASE64_ENCODED_DER_CSR"
}
```

**Response (Success):**
```json
{
  "success": true,
  "message": "enrolled",
  "data": {
    "certificate": "BASE64_ENCODED_DER_CERTIFICATE"
  }
}
```

**Response (Error):**
```json
{
  "success": false,
  "message": "Enrollment failed",
  "data": { "details": "error description" }
}
```

---

### 5. Submit Invoice for Clearance

```
POST /clear
Content-Type: application/json
X-Sandbox-Mode: true   (optional - skip validation)
```

**Request:**
```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "invoice_hash": "BASE64_SHA256_HASH",
  "invoice": "BASE64_UBL_INVOICE_XML"
}
```

**Response (Success):**
```json
{
  "success": true,
  "message": "Invoice cleared",
  "data": {
    "cleared_invoice": "BASE64_SIGNED_INVOICE"
  }
}
```

**Response (Error):**
```json
{
  "success": false,
  "message": "Clearance failed",
  "data": { "details": "error description" }
}
```

---

### 6. Submit Invoice for Reporting

```
POST /report
Content-Type: application/json
X-Sandbox-Mode: true   (optional - skip validation)
```

**Request:**
```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "invoice_hash": "BASE64_SHA256_HASH",
  "invoice": "BASE64_UBL_INVOICE_XML"
}
```

**Response (Success):**
```json
{
  "success": true,
  "message": "Invoice reported",
  "data": null
}
```

---

### 7. Verify QR Code

```
POST /verify_qr
Content-Type: application/json
```

**Request:**
```json
{
  "qr_data": "BASE64_QR_CODE_DATA"
}
```

---

## Data Models

### SubmitInvoiceDto

```json
{
  "uuid": "string (UUID v4)",
  "invoice_hash": "string (Base64 SHA256)",
  "invoice": "string (Base64 UBL Invoice XML)"
}
```

### EnrollDTO

```json
{
  "token": "string",
  "csr": "string (Base64 DER)"
}
```

### OnboardDto

```json
{
  "name": "string",
  "email": "string",
  "company_id": "string (TIN)"
}
```

### ApiResponse<T>

```json
{
  "success": "boolean",
  "message": "string",
  "data": "T | null"
}
```

---

## Invoice Submission

### Invoice Structure Requirements

The invoice must be a valid **UBL 2.1** XML document with the following elements:

1. **ProfileID**: `clearance:1.0` or `reporting:1.0`
2. **ID**: Unique invoice identifier
3. **IssueDate**: Invoice date
4. **InvoiceTypeCode**: Type of invoice
5. **DocumentCurrencyCode**: Currency code
6. **AccountingSupplierParty**: Seller information (TIN)
7. **AccountingCustomerParty**: Buyer information (TIN)
8. **Signature**: XAdES signature

### Invoice Hash Computation

1. Canonicalize the invoice using C14N 1.1
2. Compute SHA-256 hash
3. Base64 encode

```bash
# Example: Compute invoice hash
cat invoice.xml | xmllint --c14n11 - | openssl dgst -sha256 -binary | base64
```

### Signed Invoice Elements

The server adds/updates:
- `ds:SignatureValue`: Server signature
- `xades:SigningTime`: Timestamp
- `xades:HashValue`: Invoice hash
- QR code in extension (clearance mode)

---

## Validation Pipeline

### Clearance Mode

The server validates in this order:

1. **UUID Uniqueness** - Check invoice UUID doesn't exist in database
2. **Schema Validation** - XSD validation against UBL 2.1
3. **Invoice Type** - Verify profile ID (clearance:1.0)
4. **Hash Verification** - Compute and compare SHA-256
5. **PIH Chain** - Verify previous invoice hash chain
6. **Certificate** - Verify device certificate against CA
7. **Signature** - Verify client signature with device certificate
8. **Supplier TIN** - Verify supplier exists in taxpayer registry
9. **Customer TIN** - Verify customer exists (clearance mode)

### Reporting Mode

Same as clearance except:
- No PIH chain verification
- No stamping/signing

### Sandbox Mode

Skip validation using header:
```
X-Sandbox-Mode: true
```

**Skipped in sandbox:**
- UUID uniqueness check
- PIH chain verification
- Database persistence
- ICV increment

**Still validated:**
- XSD schema
- Hash computation
- Signature verification
- Certificate validation
- TIN verification

---

## Error Handling

### HTTP Status Codes

| Code | Meaning |
|------|---------|
| 200 | Success |
| 400 | Bad Request (validation failed) |
| 500 | Internal Server Error |

### Error Response Format

```json
{
  "success": false,
  "message": "Clearance failed",
  "data": {
    "details": "Detailed error message"
  }
}
```

---

## Code Examples

### Python Client Example

```python
import requests
import base64
import uuid
import hashlib
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding

BASE_URL = "http://localhost:8080"

class STCClient:
    def __init__(self, base_url: str = BASE_URL):
        self.base_url = base_url
        self.certificate = None
        self.private_key = None
        self.device_uuid = str(uuid.uuid4())
    
    def generate_token(self, name: str, email: str, tin: str) -> str:
        """Step 1: Generate enrollment token"""
        response = requests.post(
            f"{self.base_url}/onboard",
            json={"name": name, "email": email, "company_id": tin}
        )
        response.raise_for_status()
        data = response.json()
        return data["data"]["token"]
    
    def enroll(self, token: str, csr_der_b64: str) -> str:
        """Step 2: Enroll device and get certificate"""
        response = requests.post(
            f"{self.base_url}/enroll",
            json={"token": token, "csr": csr_der_b64}
        )
        response.raise_for_status()
        data = response.json()
        return data["data"]["certificate"]
    
    def compute_hash(self, invoice_xml: bytes) -> str:
        """Compute SHA-256 hash of canonicalized invoice"""
        import subprocess
        # Canonicalize using xmllint
        result = subprocess.run(
            ["xmllint", "--c14n11", "-"],
            input=invoice_xml,
            capture_output=True
        )
        canonical = result.stdout
        # Compute hash
        hash_val = hashlib.sha256(canonical).digest()
        return base64.b64encode(hash_val).decode()
    
    def submit_clearance(self, invoice_xml: bytes, sandbox: bool = False) -> bytes:
        """Submit invoice for clearance"""
        invoice_b64 = base64.b64encode(invoice_xml).decode()
        invoice_hash = self.compute_hash(invoice_xml)
        invoice_uuid = str(uuid.uuid4())
        
        headers = {"Content-Type": "application/json"}
        if sandbox:
            headers["X-Sandbox-Mode"] = "true"
        
        response = requests.post(
            f"{self.base_url}/clear",
            json={
                "uuid": invoice_uuid,
                "invoice_hash": invoice_hash,
                "invoice": invoice_b64
            },
            headers=headers
        )
        response.raise_for_status()
        data = response.json()
        
        if not data.get("success"):
            raise Exception(f"Clearance failed: {data.get('data', {}).get('details')}")
        
        cleared = data["data"]["cleared_invoice"]
        return base64.b64decode(cleared)
    
    def submit_reporting(self, invoice_xml: bytes, sandbox: bool = False) -> bool:
        """Submit invoice for reporting"""
        invoice_b64 = base64.b64encode(invoice_xml).decode()
        invoice_hash = self.compute_hash(invoice_xml)
        invoice_uuid = str(uuid.uuid4())
        
        headers = {"Content-Type": "application/json"}
        if sandbox:
            headers["X-Sandbox-Mode"] = "true"
        
        response = requests.post(
            f"{self.base_url}/report",
            json={
                "uuid": invoice_uuid,
                "invoice_hash": invoice_hash,
                "invoice": invoice_b64
            },
            headers=headers
        )
        response.raise_for_status()
        data = response.json()
        return data.get("success", False)


# Usage Example
client = STCClient()

# Enroll device (only once)
token = client.generate_token("My Company", "dev@example.com", "399999999900003")
# Generate CSR using OpenSSL, then enroll
# certificate = client.enroll(token, csr_b64)

# Submit invoice
with open("invoice.xml", "rb") as f:
    invoice_xml = f.read()

# Sandbox mode
cleared = client.submit_clearance(invoice_xml, sandbox=True)

# Production mode (requires enrollment)
# cleared = client.submit_clearance(invoice_xml, sandbox=False)

print("Invoice cleared successfully!")
```

### Node.js Client Example

```javascript
const axios = require('axios');
const crypto = require('crypto');
const fs = require('fs');

const BASE_URL = 'http://localhost:8080';

class STCClient {
  constructor() {
    this.baseUrl = BASE_URL;
    this.deviceUuid = crypto.randomUUID();
  }

  async generateToken(name, email, tin) {
    const response = await axios.post(`${this.baseUrl}/onboard`, {
      name,
      email,
      company_id: tin
    });
    return response.data.data.token;
  }

  async enroll(token, csrB64) {
    const response = await axios.post(`${this.baseUrl}/enroll`, {
      token,
      csr: csrB64
    });
    return response.data.data.certificate;
  }

  computeHash(invoiceXml) {
    const hash = crypto.createHash('sha256');
    hash.update(invoiceXml);
    return hash.digest('base64');
  }

  async submitClearance(invoiceXml, sandbox = false) {
    const invoiceB64 = Buffer.from(invoiceXml).toString('base64');
    const invoiceHash = this.computeHash(invoiceXml);
    const invoiceUuid = crypto.randomUUID();

    const headers = { 'Content-Type': 'application/json' };
    if (sandbox) headers['X-Sandbox-Mode'] = 'true';

    const response = await axios.post(`${this.baseUrl}/clear`, {
      uuid: invoiceUuid,
      invoice_hash: invoiceHash,
      invoice: invoiceB64
    }, { headers });

    if (!response.data.success) {
      throw new Error(`Clearance failed: ${response.data.data.details}`);
    }

    return Buffer.from(response.data.data.cleared_invoice, 'base64');
  }

  async submitReporting(invoiceXml, sandbox = false) {
    const invoiceB64 = Buffer.from(invoiceXml).toString('base64');
    const invoiceHash = this.computeHash(invoiceXml);
    const invoiceUuid = crypto.randomUUID();

    const headers = { 'Content-Type': 'application/json' };
    if (sandbox) headers['X-Sandbox-Mode'] = 'true';

    const response = await axios.post(`${this.baseUrl}/report`, {
      uuid: invoiceUuid,
      invoice_hash: invoiceHash,
      invoice: invoiceB64
    }, { headers });

    return response.data.success;
  }
}

// Usage
const client = new STCClient();
const invoiceXml = fs.readFileSync('invoice.xml');

// Sandbox mode
client.submitClearance(invoiceXml, true)
  .then(cleared => {
    fs.writeFileSync('cleared_invoice.xml', cleared);
    console.log('Invoice cleared!');
  })
  .catch(err => console.error(err));
```

### Rust Client Example (using reqwest)

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use uuid::Uuid;
use sha2::{Sha256, Digest};

#[derive(Serialize)]
struct SubmitInvoiceRequest {
    uuid: String,
    invoice_hash: String,
    invoice: String,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    success: bool,
    message: String,
    data: Option<T>,
}

struct STCClient {
    client: Client,
    base_url: String,
}

impl STCClient {
    fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    fn compute_hash(&self, invoice: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(invoice);
        let result = hasher.finalize();
        BASE64.encode(result)
    }

    async fn submit_clearance(&self, invoice: &[u8], sandbox: bool) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let invoice_b64 = BASE64.encode(invoice);
        let invoice_hash = self.compute_hash(invoice);
        let uuid = Uuid::new_v4().to_string();

        let mut request = self.client
            .post(format!("{}/clear", self.base_url))
            .json(&SubmitInvoiceRequest {
                uuid,
                invoice_hash,
                invoice: invoice_b64,
            });

        if sandbox {
            request = request.header("X-Sandbox-Mode", "true");
        }

        let response = request.send().await?.json::<ApiResponse<ClearanceData>>().await?;

        if !response.success {
            return Err(response.data.map(|d| d.details.into()).unwrap_or("Unknown error".into()));
        }

        Ok(BASE64.decode(response.unwrap().cleared_invoice)?)
    }
}

#[derive(Deserialize)]
struct ClearanceData {
    cleared_invoice: String,
    details: Option<String>,
}
```

### cURL Examples

```bash
# Health check
curl http://localhost:8080/health_check

# Generate token
curl -X POST http://localhost:8080/onboard \
  -H "Content-Type: application/json" \
  -d '{"name":"Test","email":"test@example.com","company_id":"399999999900003"}'

# Submit clearance (sandbox)
curl -X POST http://localhost:8080/clear \
  -H "Content-Type: application/json" \
  -H "X-Sandbox-Mode: true" \
  -d '{"uuid":"550e8400-e29b-41d4-a716-446655440000","invoice_hash":"HASH","invoice":"INVOICE_B64"}'

# Submit reporting (sandbox)
curl -X POST http://localhost:8080/report \
  -H "Content-Type: application/json" \
  -H "X-Sandbox-Mode: true" \
  -d '{"uuid":"550e8400-e29b-41d4-a716-446655440000","invoice_hash":"HASH","invoice":"INVOICE_B64"}'
```

---

## Testing

### Run Integration Tests

```bash
# Set base URL
export BASE_URL=http://localhost:8080

# Run all tests
./scripts/tests/run_all.sh

# Individual tests
./scripts/tests/test_clearance.sh
./scripts/tests/test_reporting.sh
./scripts/tests/test_enrollment.sh
```

### Test Invoice Format

Use sandbox mode to test invoice format without affecting the production chain:

```bash
curl -X POST http://localhost:8080/clear \
  -H "Content-Type: application/json" \
  -H "X-Sandbox-Mode: true" \
  -d @- << 'EOF'
{
  "uuid": "8d487816-70b8-4ade-a618-9d620b73814a",
  "invoice_hash": "ITLDpoQ8InzLMDaYuK8prsmRRjs/cLHX91STO4SVMvU=",
  "invoice": "PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz48SW52b2ljZSB4bWxucz0idXJuOm9hc2lzOm5hbWVzOnNwZWNpZmljYXRpb246dWJsOnNjaGVtYTp4c2Q6SW52b2ljZS0yIiB4bWxuczpjYmM9InVybjpvYXNpczpuYW1lczpzcGVjaWZpY2F0aW9uOnVibDpzY2hlbWE6eHNkOkNvbW1vbkJhc2ljQ29tcG9uZW50cy0yIiB4bWxuczpleHQ9InVybjpvYXNpczpuYW1lczpzcGVjaWZpY2F0aW9uOnVibDpzY2hlbWE6eHNkOkNvbW1vbkV4dGVuc2lvbkNvbXBvbmVudHMtMiI+PGNjYzpQcm9maWxlSUQ+Y2xlYXJhbmNlOjEuMDwvY2JjOlByb2ZpbGVJRD48Y2JjOklEPnNhbXBsZS1pbnZvaWNlPC9jYmM6SUQ+PGNjYjpJc3N1ZWRDYXRlPjIwMjQtMDEtMDFUMDA6MDA6MDBaPC9jYmM6SXNzdWVkRGF0ZT48Y2JjOkludm9pY2VUeXBlQ29kZSBjb2RlPSJzb2xlcyIvPjxjYmM6TG9jYWxLZXlDb2RlPmE8L2NiYzpMb2NhbEtleUNvZGU+PGNhYzpBdGNhY3R1YWxTZXR0bGVkQWNjb3VudGluZ1Bvc3RhbEFkZHJlc3M+PGNhYzpTdHJlZXROYW1lPlRlc3QgU3RyZWV0PC9jYWM6U3RyZWV0TmFtZT48Y2FjOlN0cmVldE5hbWU+PC9hY2M6U3RyZWV0TmFtZT48Y2FjOkJ1aWxkaW5nTnVtYmVyPjEyMzQ8L2NhYzpCdWlsZGluZ051bWJlcj48Y2FjOkNpdHlTdWJEaXZpc2lvbk5hbWU+PC9hY2M6Q2l0eVN1YkRpdmlzaW9uTmFtZT48Y2FjOkNpdHlOYW1lPlRlc3QgQ2l0eTwvY2FjOkNpdHlOYW1lPj48Y2FjOkNvdW50cnk+PENCBDpJZGVudGlmaWNhdGlvbkNvZGU+U0E8L0NCBDpJZGVudGlmaWNhdGlvbkNvZGU+PC9hY2M6Q291bnRyeT48L2NhYzpBY3R1YWxTZXR0bGVkQWNjb3VudGluZ1Bvc3RhbEFkZHJlc3M+PGNhYzpCdXllckFjY291bnRpbmc+PGNhYzpQYXJ0eT48Y2FjOlBhcnR5SWRlbnRpZmljYXRpb24+PGNhYzpJRD4xMjM0NTY3ODkwPC9jYWM6SUQ+PC9hY2M6UGFydHlJZGVudGlmaWNhdGlvbj48L2NhYzpQYXJ0eT48L2NhYzpCdXllckFjY291bnRpbmc+PC9JbnZvaWNlPg=="
}
EOF
```

---

## Appendix: Database Schema

### Taxpayers Table

```sql
CREATE TABLE taxpayers (
    tin VARCHAR(10) PRIMARY KEY,
    name TEXT NOT NULL,
    address TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Devices Table

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

**Initial PIH**: `5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9`

### Invoices Table

```sql
CREATE TABLE invoices (
    uuid UUID PRIMARY KEY,
    hash BYTEA NOT NULL,
    invoiceb64 TEXT,
    device_id UUID REFERENCES devices(device_uuid),
    invoice_type TEXT CHECK (invoice_type IN ('reporting', 'clearance')),
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### CSR Challenges Table

```sql
CREATE TABLE csr_challenges (
    token_hash BYTEA PRIMARY KEY,
    company_id TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '5 minutes'),
    used_at TIMESTAMPTZ
);
```

---

*Document Version: 1.0*
*Last Updated: 2026*
