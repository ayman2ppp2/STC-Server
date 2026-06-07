use actix_web::{HttpResponse, web};
use base64::{Engine, engine::general_purpose};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    errors::{ApiError, ErrorCode},
    models::{
        responses::ApiResponse,
        taxpayer_portal::{
            EnrollmentTokenDto, InvoicePayloadDto, PreparedInvoicePayloadDto,
            TaxpayerCredentialsDto, TaxpayerDto,
        },
    },
    services::{
        crypto::pki_service::compute_hash,
        db::taxpayer_auth::authenticate_taxpayer,
        pipeline::onboarding_service,
        xml::{c14n11::canonicalize_c14n11, extractors::extract_invoice},
    },
};

pub async fn sign_in(
    credentials: web::Json<TaxpayerCredentialsDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let credentials = credentials.into_inner();
    let taxpayer = authenticate_taxpayer(&credentials.tin, &credentials.password, &pool)
        .await
        .map_err(|error| {
            tracing::error!(tin = %credentials.tin, error = %error, "Taxpayer sign-in failed");
            ApiError::internal()
        })?
        .ok_or_else(|| ApiError::new(ErrorCode::InvalidCredentials))?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Signed in".to_string(),
        data: Some(TaxpayerDto {
            tin: taxpayer.tin,
            name: taxpayer.name,
        }),
    }))
}

pub async fn generate_enrollment_token(
    credentials: web::Json<TaxpayerCredentialsDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let credentials = credentials.into_inner();
    let taxpayer = authenticate_taxpayer(&credentials.tin, &credentials.password, &pool)
        .await
        .map_err(|error| {
            tracing::error!(tin = %credentials.tin, error = %error, "Taxpayer token authentication failed");
            ApiError::internal()
        })?
        .ok_or_else(|| ApiError::new(ErrorCode::InvalidCredentials))?;

    let onboarding = onboarding_service::generate_token(&taxpayer.tin, &pool)
        .await
        .map_err(|error| {
            tracing::error!(tin = %taxpayer.tin, error = %error, "Enrollment token generation failed");
            ApiError::from_token_generation(&error)
        })?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Token generated successfully. Use this token within 5 minutes.".to_string(),
        data: Some(json!(EnrollmentTokenDto {
            tin: taxpayer.tin,
            name: taxpayer.name,
            token: onboarding.token,
            expires_in_seconds: 300,
        })),
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
