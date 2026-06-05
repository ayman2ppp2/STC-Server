use actix_web::{HttpRequest, HttpResponse, web};
use fastxml::schema::CompiledSchema;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    config::crypto_config::Crypto,
    errors::{ApiError, ErrorCode},
    models::{
        responses::ApiResponse,
        submit_invoice::{InvoiceType, SubmitInvoiceDto},
    },
    services::{
        pipeline::clearance_service::process_clearance,
        pipeline::reporting_service::process_reporting,
    },
};

pub async fn get_invoices(db: web::Data<PgPool>) -> Result<HttpResponse, ApiError> {
    let result = sqlx::query(
        r#"
        SELECT
            *
        FROM invoices
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(db.get_ref())
    .await;

    match result {
        Ok(invoices) => Ok(HttpResponse::Ok().json(invoices.len())),
        Err(e) => {
            tracing::error!(?e, "DB error in get_invoices");
            Err(ApiError::internal())
        }
    }
}

pub async fn clearance(
    req: HttpRequest,
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, ApiError> {
    let sandbox = sandbox_mode(&req);
    let dto = invoice_dto.into_inner();
    let raw_uuid = dto.uuid.clone();

    let intermediate_dto = dto.parse(&db_pool).await.map_err(|e| {
        tracing::error!(uuid = %raw_uuid, error = %e, "Failed to parse clearance invoice");
        ApiError::from_invoice_parse(&e)
    })?;

    if !intermediate_dto.device.is_active {
        tracing::warn!(
            uuid = %intermediate_dto.uuid,
            device_uuid = %intermediate_dto.device.device_uuid,
            "Invoice rejected because device is inactive"
        );
        return Err(ApiError::new(ErrorCode::DeviceInactive));
    }

    let uuid = intermediate_dto.uuid;
    let device_uuid = intermediate_dto.device.device_uuid;

    let cleared_invoice = process_clearance(
        intermediate_dto,
        &db_pool,
        &crypto,
        sandbox,
        schema_validator,
        InvoiceType::Clearance,
    )
    .await
    .map_err(|e| {
        tracing::error!(uuid = %uuid, device_uuid = %device_uuid, error = %e, "Clearance pipeline failed");
        ApiError::from_invoice_pipeline(&e)
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Invoice cleared".into(),
        data: Some(json!({"cleared_invoice": cleared_invoice})),
    }))
}

pub async fn reporting(
    req: HttpRequest,
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, ApiError> {
    let sandbox = sandbox_mode(&req);
    let dto = invoice_dto.into_inner();
    let raw_uuid = dto.uuid.clone();

    let intermediate_dto = dto.parse(&db_pool).await.map_err(|e| {
        tracing::error!(uuid = %raw_uuid, error = %e, "Failed to parse reporting invoice");
        ApiError::from_invoice_parse(&e)
    })?;

    if !intermediate_dto.device.is_active {
        tracing::warn!(
            uuid = %intermediate_dto.uuid,
            device_uuid = %intermediate_dto.device.device_uuid,
            "Invoice rejected because device is inactive"
        );
        return Err(ApiError::new(ErrorCode::DeviceInactive));
    }

    let uuid = intermediate_dto.uuid;
    let device_uuid = intermediate_dto.device.device_uuid;

    process_reporting(
        intermediate_dto,
        &db_pool,
        &crypto,
        sandbox,
        schema_validator,
        InvoiceType::Reporting,
    )
    .await
    .map_err(|e| {
        tracing::error!(uuid = %uuid, device_uuid = %device_uuid, error = %e, "Reporting pipeline failed");
        ApiError::from_invoice_pipeline(&e)
    })?;

    Ok(HttpResponse::Accepted().json(ApiResponse::<()> {
        success: true,
        message: "Invoice reported".into(),
        data: None,
    }))
}

fn sandbox_mode(req: &HttpRequest) -> bool {
    req.headers()
        .get("X-Sandbox-Mode")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}
