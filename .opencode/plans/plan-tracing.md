# Full Tracing Implementation Plan

## Overview

Implement comprehensive distributed tracing with spans for the entire request lifecycle and business logic using idiomatic Rust patterns.

---

## Current State

**`src/main.rs` - Current tracing setup:**
```rust
fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("stc_server=warn"));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();
}
```

**Issues:**
- Only logs at `warn` level by default
- No request tracing (no spans)
- No structured JSON output
- No instrumentation on business logic
- Routes have manual `info!`/`error!` logging that will be redundant

---

## Desired State

- All HTTP requests traced with spans (via TracingLogger)
- All business logic steps have child spans (via `#[instrument]`)
- JSON structured logging
- Info level by default
- Trace IDs for request correlation
- No redundant logging (remove manual calls in routes)

---

## Implementation

### Phase 1: Dependencies

**Add to `Cargo.toml`:**
```toml
tracing-actix-web = "0.7"
tracing-attributes = "0.1"
tracing-subscriber = { version = "0.3", features = ["json"] }
```

Note: `tracing-attributes` provides the `#[instrument]` macro.

### Phase 2: Update `src/main.rs`

**Replace `init_tracing()` function:**
```rust
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
        )
        .init();
}
```

**Add TracingLogger middleware:**
```rust
use tracing_actix_web::TracingLogger;

HttpServer::new(move || {
    App::new()
        .wrap(TracingLogger::default())
        // ... existing routes
})
```

### Phase 3: Remove Redundant Logging from Routes

#### 3.1 `src/routes/invoice_controller.rs`

**REMOVE** these lines (already covered by TracingLogger + service spans):
```rust
// REMOVE from clearance():
info!(uuid = %uuid, endpoint = "/clear", sandbox, "Received clearance request");
error!(uuid = %uuid, endpoint = "/clear", "Failed to parse invoice DTO: {}", e);
info!(uuid = %uuid, endpoint = "/clear", "Clearance successful");
error!(uuid = %uuid, endpoint = "/clear", "Clearance failed: {}", e);

// REMOVE from reporting():
info!(uuid = %uuid, endpoint = "/report", sandbox, "Received reporting request");
error!(uuid = %uuid, endpoint = "/report", "Failed to parse invoice DTO: {}", e);
info!(uuid = %uuid, endpoint = "/report", "Reporting successful");
error!(uuid = %uuid, endpoint = "/report", "Reporting failed: {}", e);
```

**REMOVE** the unused import:
```rust
// REMOVE this line:
use tracing::{info, error};
```

#### 3.2 `src/routes/enroll.rs`

**REMOVE** any redundant logging (currently none, but keep in mind TracingLogger covers the request).

#### 3.3 `src/routes/token_generator.rs`

**REMOVE** any redundant logging (currently none, but keep in mind TracingLogger covers the request).

#### 3.4 Other routes

Other routes (`on_boarding`, `verify_qr`, `health_check`, `get_invoices`) have no manual logging - no changes needed.

### Phase 4: Instrument Service Functions with `#[instrument]`

#### 4.1 `src/services/validation_service.rs`

**Add import:**
```rust
use tracing::instrument;
```

**Add attribute to function:**
```rust
#[instrument(skip(db_pool, crypto, schema, intermediate), fields(uuid = %uuid, supplier_tin = %supplier_tin, invoice_type = ?invoice_type))]
pub async fn validate_invoice(
    intermediate: &IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: Data<CompiledSchema>,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    // Body unchanged - manual info/debug/error calls removed or kept as-is
    // They will appear as child logs of the auto-generated span
}
```

**REMOVE** manual logging calls (since `#[instrument]` already captures function entry/exit and fields):
```rust
// REMOVE these lines:
info!(uuid = %uuid, supplier_tin = %supplier_tin, invoice_type = ?invoice_type, sandbox, "Starting invoice validation");
debug!(uuid = %uuid, "Schema validation starting...");
debug!(uuid = %uuid, "Schema validation passed");
debug!(uuid = %uuid, "Hash verification passed");
debug!(uuid = %uuid, "Verifying certificate...");
debug!(uuid = %uuid, "Verifying signature...");
debug!(uuid = %uuid, supplier_tin = %supplier_tin, "Verifying supplier TIN with certificate");
debug!(uuid = %uuid, supplier_tin = %supplier_tin, "Verifying supplier TIN in database");
info!(uuid = %uuid, supplier_tin = %supplier_tin, invoice_type = ?invoice_type, "Invoice validation passed (reporting)");
info!(uuid = %uuid, supplier_tin = %supplier_tin, customer_id = %customer_id, invoice_type = ?invoice_type, "Invoice validation passed (clearance)");

// KEEP error calls (they indicate failures):
error!(uuid = %uuid, "UUID check failed: {}", e);
error!(uuid = %uuid, "Schema validation failed: {}", e);
error!(uuid = %uuid, invoice_type = ?invoice_type, "Invoice type mismatch: {}", e);
error!(uuid = %uuid, "Invoice hash mismatch - received: {:?}, computed: {:?}", received_hash, computed_hash);
error!(uuid = %uuid, device_uuid = %intermediate.device.device_uuid, "PIH verification failed: {}", e);
error!(uuid = %uuid, "Certificate verification failed: {}", e);
error!(uuid = %uuid, "Signature verification failed: {}", e);
error!(uuid = %uuid, supplier_tin = %supplier_tin, "Supplier TIN mismatch with certificate: {}", e);
error!(uuid = %uuid, supplier_tin = %supplier_tin, "Supplier TIN not found in database: {}", e);
error!(uuid = %uuid, supplier_tin = %supplier_tin, customer_id = %customer_id, "Customer TIN not found in database: {}", e);
```

**REMOVE** the unused imports:
```rust
// REMOVE these lines:
use tracing::{debug, error, info};
```

#### 4.2 `src/services/clearance_service.rs`

```rust
use tracing::instrument;

#[instrument(skip(db_pool, crypto, schema, intermediate), fields(uuid = %intermediate.uuid))]
pub async fn process_clearance(
    intermediate: IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: Data<CompiledSchema>,
    invoice_type: InvoiceType,
) -> anyhow::Result<String> {
    // validate_invoice will create its own child span
    validate_invoice(&intermediate, db_pool, crypto, sandbox, schema, invoice_type).await?;

    // Clearance-specific logic
    let (hash, cleared_invoice) = clear_invoice(&intermediate, crypto)?;

    if !sandbox {
        // Transaction operations - span will cover the entire block
        let mut tx = db_pool.begin().await?;
        let device = fetch_device_for_update(&intermediate.device.device_uuid, &mut tx).await?;
        let icv = extract_icv(&intermediate.invoice_bytes)?;
        verify_icv(icv, device.current_icv)?;
        update_icv_and_pih(&mut tx, &device.device_uuid, device.current_icv + 1, hash.clone()).await?;
        save_invoice(&mut tx, &cleared_invoice, &intermediate.uuid, hash, &device.device_uuid, InvoiceType::Clearance).await?;
        tx.commit().await?;
    }
    Ok(cleared_invoice)
}
```

#### 4.3 `src/services/reporting_service.rs`

```rust
use tracing::instrument;

#[instrument(skip(db_pool, crypto, schema, intermediate), fields(uuid = %intermediate.uuid))]
pub async fn process_reporting(
    intermediate: IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: Data<CompiledSchema>,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    validate_invoice(&intermediate, db_pool, crypto, sandbox, schema, invoice_type).await?;
    let hash = compute_hash(&intermediate.canonicalized_invoice_bytes)?;

    if !sandbox {
        let mut tx = db_pool.begin().await?;
        let device = fetch_device_for_update(&intermediate.device.device_uuid, &mut tx).await?;
        let icv = extract_icv(&intermediate.invoice_bytes)?;
        verify_icv(icv, device.current_icv)?;
        update_icv_and_pih(&mut tx, &device.device_uuid, device.current_icv + 1, hash.clone()).await?;
        save_invoice(&mut tx, &String::from_utf8(intermediate.invoice_bytes)?, &intermediate.uuid, hash, &device.device_uuid, InvoiceType::Reporting).await?;
        tx.commit().await?;
    }
    Ok(())
}
```

#### 4.4 `src/services/pki_service.rs`

```rust
use tracing::instrument;

// enroll_device is the main function to instrument
#[instrument(skip(crypto, intermediate_dto, pool), fields(device_id = ?intermediate_dto.get_device_id().ok(), tin = ?intermediate_dto.get_tin().ok()))]
pub async fn enroll_device(
    intermediate_dto: &IntermediateEnrollDto,
    crypto: &Crypto,
    pool: &PgPool,
) -> anyhow::Result<String> {
    let certificate = handle_enrollment(intermediate_dto, crypto).await?;
    let device_id_str = intermediate_dto.get_device_id()?;
    let device_uuid = Uuid::parse_str(&device_id_str).context("Failed to parse device ID as UUID")?;
    let tin = intermediate_dto.get_tin()?;
    verify_supplier_tin(tin.as_bytes(), pool).await?;
    create_new_device(&device_uuid, &tin, pool).await?;
    Ok(certificate)
}

// Also instrument handle_enrollment
#[instrument(skip(crypto, intermediate_dto), fields(has_csr = true))]
pub async fn handle_enrollment(
    intermediate_dto: &IntermediateEnrollDto,
    crypto: &Crypto,
) -> anyhow::Result<String> {
    // ... existing logic
}
```

#### 4.5 `src/services/device_service.rs`

```rust
use tracing::instrument;

#[instrument(skip(pool), fields(device_uuid = %id))]
pub async fn fetch_device(id: &Uuid, pool: &PgPool) -> anyhow::Result<Device> {
    // ...
}

#[instrument(skip(tx), fields(device_uuid = %id))]
pub async fn fetch_device_for_update<'a>(
    id: &Uuid,
    tx: &mut Transaction<'a, Postgres>,
) -> anyhow::Result<Device> {
    // ...
}

#[instrument(skip(pool), fields(tin = %tin))]
pub async fn create_new_device(
    device_uuid: &Uuid,
    tin: &str,
    pool: &PgPool,
) -> anyhow::Result<Device> {
    // ...
}
```

#### 4.6 `src/services/save_invoice.rs`

```rust
use tracing::instrument;

#[instrument(skip(tx), fields(uuid = %uuid, device_uuid = %device_id))]
pub async fn save_invoice<'a>(
    tx: &mut Transaction<'a, Postgres>,
    invoiceb64: &String,
    uuid: &Uuid,
    hash: Vec<u8>,
    device_id: &Uuid,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    // ... existing logic
}
```

#### 4.7 `src/services/token_checking.rs`

```rust
use tracing::instrument;

#[instrument(skip(pool), fields(token_provided = true))]
pub async fn fetch_token(token: &str, pool: &PgPool) -> anyhow::Result<Option<Vec<u8>>> {
    // ...
}

#[instrument(skip(pool), fields(token_hash_provided = true))]
pub async fn mark_token_used(token_hash: &[u8], pool: &PgPool) -> anyhow::Result<()> {
    // ...
}

#[instrument(skip(pool), fields(company_id = %tin))]
pub async fn validate_taxpayer_exists(tin: &str, pool: &PgPool) -> anyhow::Result<bool> {
    // ...
}

#[instrument(skip(pool), fields(count))]
pub async fn cleanup_expired_tokens(pool: &PgPool) -> anyhow::Result<u64> {
    // ...
}
```

#### 4.8 `src/services/check_uuid.rs`

```rust
use tracing::instrument;

#[instrument(skip(db_pool), fields(uuid = %uuid))]
pub async fn check_uuid(uuid: &uuid::Uuid, db_pool: &PgPool) -> anyhow::Result<()> {
    // ... existing logic
}
```

#### 4.9 `src/services/schema_validation.rs`

```rust
use tracing::instrument;

#[instrument(skip(schema), fields(schema_validation = true))]
pub fn validate_schema(schema: Data<CompiledSchema>, body: &str) -> anyhow::Result<String> {
    // ... existing logic
}
```

#### 4.10 `src/services/icv_service.rs`

```rust
use tracing::instrument;

#[instrument(fields(expected_icv = current_icv + 1, received_icv = icv))]
pub fn verify_icv(icv: i32, current_icv: i32) -> anyhow::Result<()> {
    // ... existing logic
}

#[instrument(skip(tx), fields(device_uuid = %device_id, new_icv = new_icv))]
pub async fn update_icv_and_pih<'a>(
    tx: &mut Transaction<'a, Postgres>,
    device_id: &Uuid,
    new_icv: i32,
    new_pih: Vec<u8>,
) -> anyhow::Result<()> {
    // ... existing logic
}
```

#### 4.11 `src/services/pih_service.rs`

```rust
use tracing::instrument;

#[instrument(skip(pool), fields(device_uuid = %device_id))]
pub async fn verify_pih(
    invoice: &[u8],
    pool: &PgPool,
    device_id: &Uuid,
) -> anyhow::Result<bool> {
    // ... existing logic
}
```

#### 4.12 `src/services/tin_service.rs`

```rust
use tracing::instrument;

#[instrument(skip(pool), fields(tin = %String::from_utf8_lossy(supplier_tin)))]
pub async fn verify_supplier_tin(supplier_tin: &[u8], pool: &PgPool) -> anyhow::Result<()> {
    // ... existing logic
}

#[instrument(skip(pool), fields(tin = %String::from_utf8_lossy(customer_tin)))]
pub async fn verify_customer_tin(customer_tin: &[u8], pool: &PgPool) -> anyhow::Result<()> {
    // ... existing logic
}
```

#### 4.13 `src/services/verify_qr.rs`

```rust
use tracing::instrument;

#[instrument(skip(crypto), fields(qr_length = qr_b64.len()))]
pub fn verify_qr_signature(qr_b64: &str, crypto: &Crypto) -> anyhow::Result<()> {
    // ... existing logic
}
```

### Phase 5: Update Background Task

```rust
use tracing::instrument;

#[instrument(skip(pool))]
async fn token_cleanup_loop(pool: PgPool) {
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;

        match cleanup_expired_tokens(&pool).await {
            Ok(count) if count > 0 => tracing::info!(count, "Cleaned expired tokens"),
            Ok(_) => {}
            Err(e) => tracing::error!(%e, "Token cleanup failed"),
        }
    }
}
```

**In `main()`:**
```rust
// Replace inline tokio::spawn with:
tokio::spawn(token_cleanup_loop(pool.clone()));
```

### Phase 6: Replace `println!` and `eprintln!`

| File | Replace |
|------|---------|
| `src/config/db_config.rs:18` | `tracing::debug!("DATABASE_URL configured")` |
| `src/main.rs:69` | `tracing::info!(port, "Server starting")` |
| `src/main.rs:88` | (handled by `#[instrument]` on `token_cleanup_loop`) |
| `src/main.rs:90` | (handled by `#[instrument]` on `token_cleanup_loop`) |
| `src/main.rs:52` | `tracing::error!(?e, "DB error in get_invoices")` |

### Phase 7: Update `src/main.rs` `get_invoices`

The `get_invoices` function uses `eprintln!` for DB errors - replace with tracing:
```rust
async fn get_invoices(db: web::Data<PgPool>) -> impl Responder {
    let result = sqlx::query(/* ... */)
        .fetch_all(db.get_ref())
        .await;

    match result {
        Ok(invoices) => HttpResponse::Ok().json(invoices.len()),
        Err(e) => {
            tracing::error!(?e, "DB error in get_invoices");
            HttpResponse::InternalServerError().body("Failed to fetch invoices")
        }
    }
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` | Add dependencies |
| `src/main.rs` | Update tracing init, add TracingLogger, replace prints |
| `src/routes/invoice_controller.rs` | Remove manual logging, remove unused imports |
| `src/services/validation_service.rs` | Add `#[instrument]`, remove manual info/debug calls, keep errors |
| `src/services/clearance_service.rs` | Add `#[instrument]` |
| `src/services/reporting_service.rs` | Add `#[instrument]` |
| `src/services/pki_service.rs` | Add `#[instrument]` to `enroll_device`, `handle_enrollment` |
| `src/services/device_service.rs` | Add `#[instrument]` |
| `src/services/save_invoice.rs` | Add `#[instrument]` |
| `src/services/token_checking.rs` | Add `#[instrument]` |
| `src/services/check_uuid.rs` | Add `#[instrument]` |
| `src/services/schema_validation.rs` | Add `#[instrument]` |
| `src/services/icv_service.rs` | Add `#[instrument]` |
| `src/services/pih_service.rs` | Add `#[instrument]` |
| `src/services/tin_service.rs` | Add `#[instrument]` |
| `src/services/verify_qr.rs` | Add `#[instrument]` |

---

## Example Output

**Request log (JSON):**
```json
{"level":"INFO","timestamp":"2026-04-11T10:30:00Z","target":"actix_http","message":"Request headers received","method":"POST","path":"/clear","remote_addr":"192.168.1.1","trace_id":"abc123"}
{"level":"INFO","timestamp":"2026-04-11T10:30:00Z","target":"stc_server::services::validation_service","message":"validate_invoice","uuid":"...","supplier_tin":"...","invoice_type":"Clearance","sandbox":false}
{"level":"INFO","timestamp":"2026-04-11T10:30:00Z","target":"stc_server::services::check_uuid","message":"check_uuid","uuid":"..."}
{"level":"INFO","timestamp":"2026-04-11T10:30:00Z","target":"stc_server::services::schema_validation","message":"validate_schema","schema_validation":true}
{"level":"INFO","timestamp":"2026-04-11T10:30:02Z","target":"actix_http","message":"Request completed","method":"POST","path":"/clear","status":200,"duration":2450,"trace_id":"abc123"}
```

**Error log (JSON):**
```json
{"level":"ERROR","timestamp":"2026-04-11T10:30:00Z","target":"stc_server::services::validation_service","message":"Schema validation failed","uuid":"...","error":"Invalid XML structure"}
```

---

## `#[instrument]` Macro Reference

```rust
#[instrument]                                    // Basic: span name = function name
#[instrument(name = "custom_name")]             // Custom span name
#[instrument(fields(uuid = %uuid))]            // Add fields
#[instrument(skip(pool))]                       // Exclude from fields (large/sensitive)
#[instrument(skip(pool, data), fields(x = 1))] // Both
#[instrument(level = tracing::Level::DEBUG)]    // Custom log level
```

---

## Implementation Order

1. Add dependencies to `Cargo.toml`
2. Update `init_tracing()` with JSON formatter in `main.rs`
3. Add `TracingLogger` middleware in `main.rs`
4. Add `#[instrument]` to service functions (any order):
   - `validation_service.rs`
   - `clearance_service.rs`
   - `reporting_service.rs`
   - `pki_service.rs`
   - `device_service.rs`
   - `save_invoice.rs`
   - `token_checking.rs`
   - `check_uuid.rs`
   - `schema_validation.rs`
   - `icv_service.rs`
   - `pih_service.rs`
   - `tin_service.rs`
   - `verify_qr.rs`
5. Remove redundant logging from `invoice_controller.rs`
6. Replace `println!`/`eprintln!` with tracing
7. Create `token_cleanup_loop` function with `#[instrument]`
8. Update `main.rs` to use the new cleanup function
9. Run `cargo check` to verify compilation

---

## Notes

- `TracingLogger` handles request-level spans automatically
- `#[instrument]` on service functions creates child spans
- Use `skip()` to exclude large/sensitive fields (bytes, schema objects, transactions)
- Use `fields()` to explicitly add important context
- Keep error logs - they provide valuable failure information
- Remove info/debug logs that just mark entry/exit (covered by spans)
- Spans automatically capture function arguments (unless skipped)
