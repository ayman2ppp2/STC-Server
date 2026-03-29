use actix_web::{HttpRequest, HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    config::{crypto_config::Crypto, xsd_config::SchemaValidator},
    models::{responses::ApiResponse, submit_invoice_dto::{InvoiceType, SubmitInvoiceDto}},
    services::{clearance_service::process_clearance, reporting_service::process_reporting},
};

pub async fn clearance(
    req: HttpRequest,
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<SchemaValidator>,
) -> Result<HttpResponse, actix_web::Error> {
    let sandbox = req.headers().contains_key("X-Sandbox-Mode");
    let intermediate_dto = invoice_dto.into_inner().parse(&db_pool).await.map_err(actix_web::error::ErrorBadRequest)?;

    match process_clearance(intermediate_dto, &db_pool, &crypto, sandbox, &schema_validator,InvoiceType::Clearance).await {
        Ok(cleared_invoice) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Invoice cleared".into(),
            data: Some(json!({"cleared_invoice": cleared_invoice})),
        })),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse {
            success: false,
            message: "Clearance failed".into(),
            data: Some(json!({"details": e.to_string()})),
        })),
    }
}

pub async fn reporting(
    req: HttpRequest,
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<SchemaValidator>,
) -> Result<HttpResponse, actix_web::Error> {
    let sandbox = req.headers().contains_key("X-Sandbox-Mode");
    let intermediate_dto = invoice_dto.into_inner().parse(&db_pool).await.map_err(actix_web::error::ErrorBadRequest)?;

    match process_reporting(intermediate_dto, &db_pool, &crypto, sandbox, &schema_validator,InvoiceType::Reporting).await {
        Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse::<()> {
            success: true,
            message: "Invoice reported".into(),
            data: None, // Reporting usually doesn't return a payload
        })),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse {
            success: false,
            message: "Reporting failed".into(),
            data: Some(json!({"details": e.to_string()})),
        })),
    }
}