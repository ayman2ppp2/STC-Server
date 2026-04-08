use actix_web::{HttpRequest, HttpResponse, web};
use fastxml::schema::CompiledSchema;
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, error};

use crate::{
    config::{crypto_config::Crypto},
    models::{responses::ApiResponse, submit_invoice_dto::{InvoiceType, SubmitInvoiceDto}},
    services::{clearance_service::process_clearance, reporting_service::process_reporting},
};

pub async fn clearance(
    req: HttpRequest,
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, actix_web::Error> {
    let sandbox = req.headers().contains_key("X-Sandbox-Mode");
    let dto = invoice_dto.into_inner();
    let uuid = dto.uuid.clone();
    
    info!(uuid = %uuid, endpoint = "/clear", sandbox, "Received clearance request");

    let intermediate_dto = match dto.parse(&db_pool).await {
        Ok(dto) => dto,
        Err(e) => {
            error!(uuid = %uuid, endpoint = "/clear", "Failed to parse invoice DTO: {}", e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse {
                success: false,
                message: "Invalid invoice data".into(),
                data: Some(json!({"details": e.to_string()})),
            }));
        }
    };

    match process_clearance(intermediate_dto, &db_pool, &crypto, sandbox, schema_validator, InvoiceType::Clearance).await {
        Ok(cleared_invoice) => {
            info!(uuid = %uuid, endpoint = "/clear", "Clearance successful");
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                message: "Invoice cleared".into(),
                data: Some(json!({"cleared_invoice": cleared_invoice})),
            }))
        }
        Err(e) => {
            error!(uuid = %uuid, endpoint = "/clear", "Clearance failed: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse {
                success: false,
                message: "Clearance failed".into(),
                data: Some(json!({"details": e.to_string()})),
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
    let uuid = dto.uuid.clone();
    
    info!(uuid = %uuid, endpoint = "/report", sandbox, "Received reporting request");

    let intermediate_dto = match dto.parse(&db_pool).await {
        Ok(dto) => dto,
        Err(e) => {
            error!(uuid = %uuid, endpoint = "/report", "Failed to parse invoice DTO: {}", e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse {
                success: false,
                message: "Invalid invoice data".into(),
                data: Some(json!({"details": e.to_string()})),
            }));
        }
    };

    match process_reporting(intermediate_dto, &db_pool, &crypto, sandbox, schema_validator, InvoiceType::Reporting).await {
        Ok(_) => {
            info!(uuid = %uuid, endpoint = "/report", "Reporting successful");
            Ok(HttpResponse::Ok().json(ApiResponse::<()> {
                success: true,
                message: "Invoice reported".into(),
                data: None,
            }))
        }
        Err(e) => {
            error!(uuid = %uuid, endpoint = "/report", "Reporting failed: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse {
                success: false,
                message: "Reporting failed".into(),
                data: Some(json!({"details": e.to_string()})),
            }))
        }
    }
}
