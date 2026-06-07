use actix_session::Session;
use actix_web::{HttpResponse, web};
use base64::{Engine, engine::general_purpose};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    errors::{ApiError, ErrorCode},
    models::{
        responses::ApiResponse,
        taxpayer_portal::{
            EnrollmentTokenDto, InvoicePayloadDto, InvoiceReportDto, InvoiceReportRowDto,
            InvoiceReportSummaryDto, PreparedInvoicePayloadDto, TaxpayerCredentialsDto,
            TaxpayerDto, TaxpayerProfileDto,
        },
    },
    services::{
        crypto::pki_service::compute_hash,
        db::taxpayer_auth::{authenticate_taxpayer, fetch_taxpayer_profile},
        pipeline::onboarding_service,
        xml::{c14n11::canonicalize_c14n11, extractors::extract_invoice},
    },
};

const INVOICE_REPORT_LIMIT: i64 = 10;

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

fn require_tin(session: &Session) -> Result<String, ApiError> {
    session
        .get::<String>("tin")
        .map_err(|_| ApiError::internal())?
        .ok_or_else(|| ApiError::new(ErrorCode::Unauthenticated))
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
    credentials: web::Json<TaxpayerCredentialsDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let tin = if let Ok(Some(t)) = session.get::<String>("tin") {
        t
    } else {
        let credentials = credentials.into_inner();
        let tin = credentials.tin.ok_or_else(|| {
            tracing::error!("Invoice report missing TIN");
            ApiError::new(ErrorCode::InvalidCredentials)
        })?;
        let password = credentials.password.ok_or_else(|| {
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
        ORDER BY created_at_raw DESC
        LIMIT $2
        "#,
    )
    .bind(&tin)
    .bind(INVOICE_REPORT_LIMIT)
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
            latest_limit: INVOICE_REPORT_LIMIT as usize,
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
