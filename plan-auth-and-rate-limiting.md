# Implementation Plan: API Key Authentication + Rate Limiting

## Overview

Add API key authentication to protect invoice endpoints, and rate limiting to prevent abuse. Designed with Redis in mind for easy future migration.

---

## Phase 1: API Key Authentication

### 1.1 Database Migration

**File:** `migrations/YYYYMMDDHHMMSS_add_api_key_to_devices.sql`

```sql
ALTER TABLE devices ADD COLUMN api_key_hash BYTEA;
CREATE INDEX idx_devices_api_key_hash ON devices(api_key_hash);
```

### 1.2 Dependencies

**Add to `Cargo.toml`:**
```toml
sha2 = "0.10"
rand = "0.8"
```

### 1.3 New Module Structure

```
src/middleware/
├── mod.rs
├── api_key_auth.rs
└── rate_limit.rs
```

### 1.4 `src/middleware/mod.rs`

```rust
pub mod api_key_auth;
pub mod rate_limit;
```

### 1.5 `src/middleware/api_key_auth.rs`

**Responsibilities:**
- Extract `X-API-Key` header from request
- SHA-256 hash the incoming key
- Lookup device by `api_key_hash` in database
- Verify `is_active = true`
- Store `device_uuid` in request extensions on success
- Return 401 if key missing/invalid, 403 if device inactive

**Response on failure:**
```json
{
  "success": false,
  "message": "Invalid API key"
}
```

### 1.6 Update `src/models/device.rs`

```rust
pub struct Device {
    pub device_uuid: Uuid,
    pub tin: String,
    pub current_icv: i32,
    pub last_pih: Vec<u8>,
    pub is_active: bool,
    pub onboarded_at: OffsetDateTime,
    pub api_key_hash: Option<Vec<u8>>,  // Add this
}
```

### 1.7 Update `src/services/device_service.rs`

Add function to save API key hash after enrollment:
```rust
pub async fn save_api_key_hash(
    device_uuid: &Uuid,
    api_key_hash: &[u8],
    pool: &PgPool,
) -> anyhow::Result<()>
```

### 1.8 Update `src/routes/enroll.rs`

After successful enrollment:
1. Generate 32-byte random key → hex string (64 chars)
2. SHA-256 hash the key
3. Save hash to database
4. Return key in response (one-time display)

**Response:**
```json
{
  "success": true,
  "message": "enrolled",
  "data": {
    "certificate": "...",
    "apiKey": "abc123..."  // Show this once to user
  }
}
```

### 1.9 Update `src/main.rs`

Apply middleware to protected routes:

| Route | Auth |
|-------|------|
| `POST /clear` | ✅ X-API-Key |
| `POST /report` | ✅ X-API-Key |
| `GET /get_invoices` | ✅ X-API-Key |
| `POST /enroll` | Token only (unchanged) |
| `GET /health_check` | ❌ Public |
| `GET /onboard` | ❌ Public |
| `POST /onboard` | ❌ Public |
| `POST /verify_qr` | ❌ Public |

```rust
HttpServer::new(move || {
    App::new()
        .app_data(xsd_schema.clone())
        .app_data(pool_data.clone())
        .app_data(crypto_data.clone())
        .app_data(web::JsonConfig::default().limit(256 * 1024))
        
        // Protected routes (require API key)
        .service(
            web::scope("/")
                .wrap(api_key_auth_middleware())
                .route("/clear", web::post().to(clearance))
                .route("/report", web::post().to(reporting))
                .route("/get_invoices", web::get().to(get_invoices))
        )
        
        // Public routes (no API key required)
        .route("/enroll", web::post().to(enroll))
        .route("/health_check", web::get().to(health_check))
        .route("/onboard", web::get().to(on_board))
        .route("/onboard", web::post().to(token_generator))
        .route("/verify_qr", web::post().to(verify_qr))
        .route("/", web::get().to(hello))
})
```

---

## Phase 2: Rate Limiting

### 2.1 Dependencies

**Add to `Cargo.toml`:**
```toml
governor = "0.7"
```

### 2.2 Structure for Redis Migration

**`src/middleware/rate_limit.rs`:**

```rust
#[cfg(feature = "redis")]
use governor::backend::redis::RedisStore;

#[cfg(not(feature = "redis"))]
use governor::backend::in_memory::InMemoryStore;

// For prototype: InMemoryStore
// For production: swap to RedisStore with feature flag
```

### 2.3 Rate Limits

| Route | Limit | Key |
|-------|-------|-----|
| `/clear`, `/report` | 100/min | device_uuid |
| `/enroll` | 5/min | IP |
| `/onboard` (POST) | 10/min | IP |
| `/get_invoices` | 60/min | device_uuid |

### 2.4 Rate Limit Response (HTTP 429)

```json
{
  "success": false,
  "message": "Too many requests"
}
```

### 2.5 Apply to Routes in `main.rs`

```rust
.route("/clear", web::post().to(clearance)
    .wrap(rate_limit_clear()))
.route("/report", web::post().to(reporting)
    .wrap(rate_limit_report()))
.route("/enroll", web::post().to(enroll)
    .wrap(rate_limit_enroll()))
.route("/get_invoices", web::get().to(get_invoices)
    .wrap(rate_limit_invoices()))
```

---

## Phase 3: Cleanup

### 3.1 Remove Dead Code

- Delete `src/services/submit_invoice_service.rs`
- Delete `src/models/submit_invoice_response_dto.rs`
- Update `src/services/mod.rs` - remove `pub mod submit_invoice_service;`
- Update `src/models/mod.rs` - remove `pub mod submit_invoice_response_dto;`

### 3.2 Fix Typos

- `src/services/editors.rs:63` - "signging time" → "signing time"
- `src/models/qr_verification_model.rs` - "QrVerificationRsponse" → "QrVerificationResponse"
- `src/services/pki_service.rs:118` - "verfiy" → "verify"

---

## Implementation Order

1. **Phase 1.1** - Create database migration
2. **Phase 1.2** - Add dependencies
3. **Phase 1.3-1.4** - Create middleware module structure
4. **Phase 1.5** - Implement API key auth middleware
5. **Phase 1.6** - Update Device model
6. **Phase 1.7** - Add save_api_key_hash function
7. **Phase 1.8** - Update enroll.rs to generate API key
8. **Phase 1.9** - Apply middleware in main.rs
9. **Phase 2** - Implement rate limiting
10. **Phase 3** - Cleanup

---

## Notes

- API key is 32 random bytes → 64 character hex string
- Store SHA-256 hash of API key in database (never plain text)
- Show API key to user only once during enrollment
- Rate limiting uses in-memory store for prototype, designed for easy Redis migration
