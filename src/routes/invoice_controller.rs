use actix_web::{HttpRequest, HttpResponse, web};
use fastxml::schema::CompiledSchema;
use serde_json::json;
use sqlx::PgPool;
use tracing;

use crate::{
    config::{crypto_config::Crypto},
    models::{responses::ApiResponse, submit_invoice::{InvoiceType, SubmitInvoiceDto}},
    services::{pipeline::clearance_service::process_clearance, pipeline::reporting_service::process_reporting},
};

pub async fn get_invoices(db: web::Data<PgPool>) -> impl actix_web::Responder {
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
        Ok(invoices) => HttpResponse::Ok().json(invoices.len()),
        Err(e) => {
            tracing::error!(?e, "DB error in get_invoices");
            HttpResponse::InternalServerError().body("Failed to fetch invoices")
        }
    }
}

pub async fn clearance(
    req: HttpRequest,
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, actix_web::Error> {
    let sandbox = req.headers().contains_key("X-Sandbox-Mode");
    let dto = invoice_dto.into_inner();
    let raw_uuid = dto.uuid.clone();

    let intermediate_dto = match dto.parse(&db_pool).await {
        Ok(dto) => dto,
        Err(e) => {
            tracing::error!(uuid = %raw_uuid, error = %e, "Failed to parse clearance invoice");
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                success: false,
                message: "Invalid invoice data".into(),
                data: None,
            }));
        }
    };
    if !intermediate_dto.device.is_active {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
            success: false,
            message: "Device is not enabled".into(),
            data: None,
        }));
    }

    let uuid = intermediate_dto.uuid;
    let device_uuid = intermediate_dto.device.device_uuid;

    match process_clearance(intermediate_dto, &db_pool, &crypto, sandbox, schema_validator, InvoiceType::Clearance).await {
        Ok(cleared_invoice) => {
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                message: "Invoice cleared".into(),
                data: Some(json!({"cleared_invoice": cleared_invoice})),
            }))
        }
        Err(e) => {
            tracing::error!(uuid = %uuid, device_uuid = %device_uuid, error = %e, "Clearance pipeline failed");
            Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                success: false,
                message: "Clearance failed".into(),
                data: Some(json!({"error": e.to_string()})),
            }))
        }
    }
}

pub async fn reporting(
    req: HttpRequest,
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, actix_web::Error> {
    let sandbox = req.headers().contains_key("X-Sandbox-Mode");
    let dto = invoice_dto.into_inner();
    let raw_uuid = dto.uuid.clone();

    let intermediate_dto = match dto.parse(&db_pool).await {
        Ok(dto) => dto,
        Err(e) => {
            tracing::error!(uuid = %raw_uuid, error = %e, "Failed to parse reporting invoice");
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                success: false,
                message: "Invalid invoice data".into(),
                data: None,
            }));
        }
    };

    if !intermediate_dto.device.is_active {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
            success: false,
            message: "Device is not enabled".into(),
            data: None,
        }));
    }

    let uuid = intermediate_dto.uuid;
    let device_uuid = intermediate_dto.device.device_uuid;

    match process_reporting(intermediate_dto, &db_pool, &crypto, sandbox, schema_validator, InvoiceType::Reporting).await {
        Ok(_) => {
            Ok(HttpResponse::Ok().json(ApiResponse::<()> {
                success: true,
                message: "Invoice reported".into(),
                data: None,
            }))
        }
        Err(e) => {
            tracing::error!(uuid = %uuid, device_uuid = %device_uuid, error = %e, "Reporting pipeline failed");
            Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                success: false,
                message: "Reporting failed".into(),
                data: None,
            }))
        }
    }
}
