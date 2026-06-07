use actix_session::Session;
use actix_web::{HttpResponse, web};
use base64::{Engine, engine::general_purpose};
use chrono::NaiveDate;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    errors::{ApiError, ErrorCode},
    models::{
        responses::ApiResponse,
        taxpayer_portal::{
            EnrollmentTokenDto, InvoicePayloadDto, InvoiceReportDto, InvoiceReportRequestDto,
            InvoiceReportRowDto, InvoiceReportSummaryDto, PreparedInvoicePayloadDto,
            TaxpayerCredentialsDto, TaxpayerDto, TaxpayerProfileDto,
        },
    },
    services::{
        crypto::pki_service::compute_hash,
        db::taxpayer_auth::{authenticate_taxpayer, fetch_taxpayer_profile},
        pipeline::onboarding_service,
        xml::{c14n11::canonicalize_c14n11, extractors::extract_invoice},
    },
};

const DEFAULT_INVOICE_REPORT_LIMIT: i64 = 25;
const MAX_INVOICE_REPORT_LIMIT: i64 = 100;

#[derive(sqlx::FromRow)]
struct InvoiceReportSummaryRow {
    total: i64,
    successful: i64,
    failed: i64,
    clearance_successful: i64,
    clearance_failed: i64,
    reporting_successful: i64,
    reporting_failed: i64,
    devices: i64,
    latest_invoice_at: Option<String>,
}

struct InvoiceReportFilters {
    status: String,
    invoice_type: String,
    search: Option<String>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    limit: i64,
    offset: i64,
}

fn require_tin(session: &Session) -> Result<String, ApiError> {
    session
        .get::<String>("tin")
        .map_err(|_| ApiError::internal())?
        .ok_or_else(|| ApiError::new(ErrorCode::Unauthenticated))
}

fn parse_report_filters(
    request: &InvoiceReportRequestDto,
) -> Result<InvoiceReportFilters, ApiError> {
    let status =
        normalize_report_choice(request.status.as_deref(), &["all", "successful", "failed"])?;
    let invoice_type = normalize_report_choice(
        request.invoice_type.as_deref(),
        &["all", "clearance", "reporting"],
    )?;
    let search = trimmed_optional(request.search.as_deref());
    let from = parse_report_date(request.from.as_deref())?;
    let to = parse_report_date(request.to.as_deref())?;

    if let (Some(from), Some(to)) = (from, to)
        && from > to
    {
        return Err(ApiError::new(ErrorCode::InvalidRequestBody));
    }

    let limit = request
        .limit
        .unwrap_or(DEFAULT_INVOICE_REPORT_LIMIT)
        .clamp(1, MAX_INVOICE_REPORT_LIMIT);
    let offset = request.offset.unwrap_or(0).max(0);

    Ok(InvoiceReportFilters {
        status,
        invoice_type,
        search,
        from,
        to,
        limit,
        offset,
    })
}

fn normalize_report_choice(value: Option<&str>, allowed: &[&str]) -> Result<String, ApiError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("all")
        .to_ascii_lowercase();

    if allowed.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(ApiError::new(ErrorCode::InvalidRequestBody))
    }
}

fn trimmed_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_report_date(value: Option<&str>) -> Result<Option<NaiveDate>, ApiError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| ApiError::new(ErrorCode::InvalidRequestBody))
}

pub async fn sign_in(
    session: Session,
    credentials: web::Json<TaxpayerCredentialsDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let credentials = credentials.into_inner();
    let tin = credentials.tin.ok_or_else(|| {
        tracing::error!("Sign-in missing TIN");
        ApiError::new(ErrorCode::InvalidCredentials)
    })?;
    let password = credentials.password.ok_or_else(|| {
        tracing::error!("Sign-in missing password");
        ApiError::new(ErrorCode::InvalidCredentials)
    })?;
    let taxpayer = authenticate_taxpayer(&tin, &password, &pool)
        .await
        .map_err(|error| {
            tracing::error!(tin = %tin, error = %error, "Taxpayer sign-in failed");
            ApiError::internal()
        })?
        .ok_or_else(|| ApiError::new(ErrorCode::InvalidCredentials))?;

    session
        .insert("tin", &taxpayer.tin)
        .map_err(|_| ApiError::internal())?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Signed in".to_string(),
        data: Some(TaxpayerDto {
            tin: taxpayer.tin,
            name: taxpayer.name,
        }),
    }))
}

pub async fn taxpayer_me(
    session: Session,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let tin = require_tin(&session)?;

    let profile = fetch_taxpayer_profile(&tin, &pool)
        .await
        .map_err(|error| {
            tracing::error!(tin = %tin, error = %error, "Failed to fetch taxpayer profile");
            ApiError::internal()
        })?
        .ok_or_else(|| {
            tracing::error!(tin = %tin, "Taxpayer not found after session authentication");
            ApiError::new(ErrorCode::CompanyIdNotRegistered)
        })?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "OK".to_string(),
        data: Some(TaxpayerProfileDto {
            tin: profile.tin,
            name: profile.name,
            address: profile.address,
            created_at: profile.created_at,
        }),
    }))
}

pub async fn sign_out(session: Session) -> HttpResponse {
    session.purge();
    HttpResponse::Found()
        .append_header(("Location", "/e-invoicing/login"))
        .finish()
}

pub async fn generate_enrollment_token(
    session: Session,
    credentials: web::Json<TaxpayerCredentialsDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let (tin, name) = if let Ok(Some(t)) = session.get::<String>("tin") {
        let profile = fetch_taxpayer_profile(&t, &pool)
            .await
            .map_err(|_| ApiError::internal())?
            .ok_or_else(|| ApiError::new(ErrorCode::CompanyIdNotRegistered))?;
        (profile.tin, profile.name)
    } else {
        let credentials = credentials.into_inner();
        let tin = credentials.tin.ok_or_else(|| {
            tracing::error!("Token generation missing TIN");
            ApiError::new(ErrorCode::InvalidCredentials)
        })?;
        let password = credentials.password.ok_or_else(|| {
            tracing::error!("Token generation missing password");
            ApiError::new(ErrorCode::InvalidCredentials)
        })?;
        let taxpayer = authenticate_taxpayer(&tin, &password, &pool)
            .await
            .map_err(|error| {
                tracing::error!(tin = %tin, error = %error, "Taxpayer token authentication failed");
                ApiError::internal()
            })?
            .ok_or_else(|| ApiError::new(ErrorCode::InvalidCredentials))?;
        (taxpayer.tin, taxpayer.name)
    };

    let onboarding = onboarding_service::generate_token(&tin, &pool)
        .await
        .map_err(|error| {
            tracing::error!(tin = %tin, error = %error, "Enrollment token generation failed");
            ApiError::from_token_generation(&error)
        })?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Token generated successfully. Use this token within 5 minutes.".to_string(),
        data: Some(json!(EnrollmentTokenDto {
            tin: tin.clone(),
            name,
            token: onboarding.token,
            expires_in_seconds: 300,
        })),
    }))
}

pub async fn invoice_report(
    session: Session,
    request: web::Json<InvoiceReportRequestDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let request = request.into_inner();
    let filters = parse_report_filters(&request)?;

    let tin = if let Ok(Some(t)) = session.get::<String>("tin") {
        t
    } else {
        let tin = request.tin.ok_or_else(|| {
            tracing::error!("Invoice report missing TIN");
            ApiError::new(ErrorCode::InvalidCredentials)
        })?;
        let password = request.password.ok_or_else(|| {
            tracing::error!("Invoice report missing password");
            ApiError::new(ErrorCode::InvalidCredentials)
        })?;
        let taxpayer = authenticate_taxpayer(&tin, &password, &pool)
            .await
            .map_err(|error| {
                tracing::error!(tin = %tin, error = %error, "Taxpayer invoice report authentication failed");
                ApiError::internal()
            })?
            .ok_or_else(|| ApiError::new(ErrorCode::InvalidCredentials))?;
        taxpayer.tin
    };

    let summary = sqlx::query_as::<_, InvoiceReportSummaryRow>(
        r#"
        WITH submissions AS (
            SELECT
                i.invoice_type,
                i.device_id,
                i.created_at,
                'successful' AS status
            FROM invoices i
            INNER JOIN devices d ON d.device_uuid = i.device_id
            WHERE d.tin = $1

            UNION ALL

            SELECT
                r.invoice_type,
                r.device_id,
                r.created_at,
                'failed' AS status
            FROM rejected_invoices r
            LEFT JOIN devices d ON d.device_uuid = r.device_id
            WHERE d.tin = $1
               OR (r.device_id IS NULL AND r.supplier_tin = $1)
        )
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE status = 'successful') AS successful,
            COUNT(*) FILTER (WHERE status = 'failed') AS failed,
            COUNT(*) FILTER (WHERE status = 'successful' AND invoice_type = 'clearance') AS clearance_successful,
            COUNT(*) FILTER (WHERE status = 'failed' AND invoice_type = 'clearance') AS clearance_failed,
            COUNT(*) FILTER (WHERE status = 'successful' AND invoice_type = 'reporting') AS reporting_successful,
            COUNT(*) FILTER (WHERE status = 'failed' AND invoice_type = 'reporting') AS reporting_failed,
            COUNT(DISTINCT device_id) AS devices,
            to_char(MAX(created_at) AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS latest_invoice_at
        FROM submissions
        "#,
    )
    .bind(&tin)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(tin = %tin, error = %error, "Failed to fetch taxpayer invoice report summary");
        ApiError::internal()
    })?;

    let filtered_total = sqlx::query_scalar::<_, i64>(
        r#"
        WITH submissions AS (
            SELECT
                i.uuid::TEXT AS uuid,
                COALESCE(i.invoice_type, 'unknown') AS invoice_type,
                i.device_id::TEXT AS device_id,
                encode(i.hash, 'hex') AS hash_value,
                i.created_at AS created_at_raw,
                'successful' AS status,
                NULL::TEXT AS error_code,
                NULL::TEXT AS error_message
            FROM invoices i
            INNER JOIN devices d ON d.device_uuid = i.device_id
            WHERE d.tin = $1

            UNION ALL

            SELECT
                r.submitted_uuid AS uuid,
                r.invoice_type,
                r.device_id::TEXT AS device_id,
                r.submitted_invoice_hash AS hash_value,
                r.created_at AS created_at_raw,
                'failed' AS status,
                r.error_code,
                r.error_message
            FROM rejected_invoices r
            LEFT JOIN devices d ON d.device_uuid = r.device_id
            WHERE d.tin = $1
               OR (r.device_id IS NULL AND r.supplier_tin = $1)
        )
        SELECT COUNT(*)
        FROM submissions
        WHERE ($2 = 'all' OR status = $2)
          AND ($3 = 'all' OR invoice_type = $3)
          AND (
              $4::TEXT IS NULL
              OR uuid ILIKE '%' || $4 || '%'
              OR COALESCE(device_id, '') ILIKE '%' || $4 || '%'
              OR hash_value ILIKE '%' || $4 || '%'
              OR COALESCE(error_code, '') ILIKE '%' || $4 || '%'
              OR COALESCE(error_message, '') ILIKE '%' || $4 || '%'
          )
          AND ($5::DATE IS NULL OR created_at_raw >= $5::DATE)
          AND ($6::DATE IS NULL OR created_at_raw < ($6::DATE + INTERVAL '1 day'))
        "#,
    )
    .bind(&tin)
    .bind(&filters.status)
    .bind(&filters.invoice_type)
    .bind(&filters.search)
    .bind(filters.from)
    .bind(filters.to)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(tin = %tin, error = %error, "Failed to count taxpayer invoice report");
        ApiError::internal()
    })?;

    let invoices = sqlx::query_as::<_, InvoiceReportRowDto>(
        r#"
        SELECT
            uuid,
            invoice_type,
            device_id,
            hash_value,
            created_at,
            status,
            error_code,
            error_message
        FROM (
            SELECT
                i.uuid::TEXT AS uuid,
                COALESCE(i.invoice_type, 'unknown') AS invoice_type,
                i.device_id::TEXT AS device_id,
                encode(i.hash, 'hex') AS hash_value,
                to_char(i.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at,
                i.created_at AS created_at_raw,
                'successful' AS status,
                NULL::TEXT AS error_code,
                NULL::TEXT AS error_message
            FROM invoices i
            INNER JOIN devices d ON d.device_uuid = i.device_id
            WHERE d.tin = $1

            UNION ALL

            SELECT
                r.submitted_uuid AS uuid,
                r.invoice_type,
                r.device_id::TEXT AS device_id,
                r.submitted_invoice_hash AS hash_value,
                to_char(r.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at,
                r.created_at AS created_at_raw,
                'failed' AS status,
                r.error_code,
                r.error_message
            FROM rejected_invoices r
            LEFT JOIN devices d ON d.device_uuid = r.device_id
            WHERE d.tin = $1
               OR (r.device_id IS NULL AND r.supplier_tin = $1)
        ) invoice_report
        WHERE ($2 = 'all' OR status = $2)
          AND ($3 = 'all' OR invoice_type = $3)
          AND (
              $4::TEXT IS NULL
              OR uuid ILIKE '%' || $4 || '%'
              OR COALESCE(device_id, '') ILIKE '%' || $4 || '%'
              OR hash_value ILIKE '%' || $4 || '%'
              OR COALESCE(error_code, '') ILIKE '%' || $4 || '%'
              OR COALESCE(error_message, '') ILIKE '%' || $4 || '%'
          )
          AND ($5::DATE IS NULL OR created_at_raw >= $5::DATE)
          AND ($6::DATE IS NULL OR created_at_raw < ($6::DATE + INTERVAL '1 day'))
        ORDER BY created_at_raw DESC
        LIMIT $7
        OFFSET $8
        "#,
    )
    .bind(&tin)
    .bind(&filters.status)
    .bind(&filters.invoice_type)
    .bind(&filters.search)
    .bind(filters.from)
    .bind(filters.to)
    .bind(filters.limit)
    .bind(filters.offset)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|error| {
        tracing::error!(tin = %tin, error = %error, "Failed to fetch taxpayer invoice report");
        ApiError::internal()
    })?;

    let summary = InvoiceReportSummaryDto {
        total: summary.total as usize,
        successful: summary.successful as usize,
        failed: summary.failed as usize,
        clearance_successful: summary.clearance_successful as usize,
        clearance_failed: summary.clearance_failed as usize,
        reporting_successful: summary.reporting_successful as usize,
        reporting_failed: summary.reporting_failed as usize,
        devices: summary.devices as usize,
        latest_invoice_at: summary.latest_invoice_at,
    };

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Invoice report loaded".to_string(),
        data: Some(InvoiceReportDto {
            summary,
            invoices,
            filtered_total: filtered_total as usize,
            limit: filters.limit as usize,
            offset: filters.offset as usize,
            has_next: filters.offset + filters.limit < filtered_total,
            has_previous: filters.offset > 0,
        }),
    }))
}

pub async fn prepare_invoice_payload(
    payload: web::Json<InvoicePayloadDto>,
) -> Result<HttpResponse, ApiError> {
    let invoice_xml = payload.into_inner().invoice_xml;
    let invoice_bytes = invoice_xml.into_bytes();
    let canonical_invoice =
        canonicalize_c14n11(extract_invoice(&invoice_bytes).map_err(|error| {
            tracing::error!(error = %error, "Sandbox invoice extraction failed");
            ApiError::new(ErrorCode::InvalidInvoiceXml)
        })?)
        .map_err(|error| {
            tracing::error!(error = %error, "Sandbox invoice canonicalization failed");
            ApiError::new(ErrorCode::InvalidInvoiceXml)
        })?;
    let invoice_hash = compute_hash(&canonical_invoice).map_err(|error| {
        tracing::error!(error = %error, "Sandbox invoice hashing failed");
        ApiError::internal()
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Invoice payload prepared".to_string(),
        data: Some(PreparedInvoicePayloadDto {
            invoice: general_purpose::STANDARD.encode(invoice_bytes),
            invoice_hash: general_purpose::STANDARD.encode(invoice_hash),
        }),
    }))
}
